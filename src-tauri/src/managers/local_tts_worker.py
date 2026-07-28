"""AivoRelay local Qwen3-TTS worker.

The protocol is JSON Lines on stdout. All third-party output is redirected to
stderr so imports and model generation cannot corrupt protocol messages.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import json
import os
from pathlib import Path
import sys
import traceback


PROTOCOL_VERSION = 1


def emit(payload: dict) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def fail(request_id: str | None, message: str, retryable: bool = False) -> None:
    emit(
        {
            "type": "error",
            "protocol": PROTOCOL_VERSION,
            "id": request_id,
            "message": message,
            "retryable": retryable,
        }
    )


def contained_path(root: Path, candidate: str) -> Path:
    path = Path(candidate).resolve()
    try:
        path.relative_to(root)
    except ValueError as error:
        raise ValueError("Output path is outside the managed worker directory") from error
    return path


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--model", required=True)
    parser.add_argument("--output-root", required=True)
    parser.add_argument("--device", choices=("cuda", "cpu"), required=True)
    args = parser.parse_args()

    model_path = Path(args.model).resolve()
    output_root = Path(args.output_root).resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    if not model_path.is_dir():
        fail(None, "The managed Qwen3-TTS model directory is missing")
        return 2

    # Never allow Transformers/Hugging Face to fetch missing files while
    # synthesis is running. Installation is the only network-enabled phase.
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"

    try:
        with contextlib.redirect_stdout(sys.stderr):
            import numpy as np
            import soundfile as sf
            import torch
            from qwen_tts import Qwen3TTSModel

            if args.device == "cuda":
                if not torch.cuda.is_available():
                    raise RuntimeError(
                        "The CUDA runtime was installed, but PyTorch cannot access a CUDA GPU"
                    )
                model = Qwen3TTSModel.from_pretrained(
                    str(model_path),
                    device_map="cuda:0",
                    dtype=torch.bfloat16,
                    attn_implementation="sdpa",
                )
            else:
                model = Qwen3TTSModel.from_pretrained(
                    str(model_path),
                    device_map="cpu",
                    dtype=torch.float32,
                    attn_implementation="sdpa",
                )
    except Exception as error:  # startup errors are terminal for this worker
        traceback.print_exc(file=sys.stderr)
        fail(None, f"Failed to load Qwen3-TTS: {error}")
        return 3

    emit(
        {
            "type": "ready",
            "protocol": PROTOCOL_VERSION,
            "device": args.device,
            "sample_rate": 24000,
        }
    )

    for raw_line in sys.stdin:
        request_id: str | None = None
        try:
            request = json.loads(raw_line)
            request_id = str(request.get("id", ""))
            if request.get("protocol") != PROTOCOL_VERSION:
                raise ValueError("Unsupported worker protocol version")
            if request.get("type") == "shutdown":
                emit(
                    {
                        "type": "shutdown",
                        "protocol": PROTOCOL_VERSION,
                        "id": request_id,
                    }
                )
                return 0
            if request.get("type") != "synthesize":
                raise ValueError("Unsupported worker request type")

            encoded_text = request.get("text_b64")
            if not isinstance(encoded_text, str):
                raise ValueError("Synthesis text transport is missing")
            text = base64.b64decode(encoded_text, validate=True).decode("utf-8")
            speaker = request.get("speaker")
            language = request.get("language") or "Auto"
            instruct = request.get("instruct") or ""
            speed = float(request.get("speed", 1.0))
            if not isinstance(text, str) or not text:
                raise ValueError("Synthesis text must not be empty")
            if not isinstance(speaker, str) or not speaker:
                raise ValueError("A supported Qwen speaker is required")
            if not 0.5 <= speed <= 2.0:
                raise ValueError("Local TTS speed must be between 0.5 and 2.0")

            output_path = contained_path(output_root, request["output_path"])
            output_path.parent.mkdir(parents=True, exist_ok=True)
            if output_path.exists():
                output_path.unlink()

            with contextlib.redirect_stdout(sys.stderr):
                max_new_tokens = min(4096, max(256, len(text) * 8))
                wavs, sample_rate = model.generate_custom_voice(
                    text=text,
                    speaker=speaker,
                    language=language,
                    instruct=instruct,
                    non_streaming_mode=True,
                    max_new_tokens=max_new_tokens,
                )
                if len(wavs) != 1:
                    raise RuntimeError("Qwen3-TTS returned an unexpected audio batch")
                samples = np.asarray(wavs[0], dtype=np.float32).reshape(-1)
                if speed != 1.0:
                    # qwen-tts already depends on librosa. Time-stretch preserves
                    # pitch while applying AivoRelay's shared speed setting.
                    import librosa

                    samples = librosa.effects.time_stretch(samples, rate=speed)
                if samples.size == 0 or not np.isfinite(samples).all():
                    raise RuntimeError("Qwen3-TTS returned invalid audio samples")
                sf.write(
                    str(output_path),
                    samples,
                    int(sample_rate),
                    format="WAV",
                    subtype="PCM_16",
                )

            emit(
                {
                    "type": "result",
                    "protocol": PROTOCOL_VERSION,
                    "id": request_id,
                    "output_path": str(output_path),
                    "sample_rate": int(sample_rate),
                    "samples": int(samples.size),
                }
            )
        except Exception as error:
            traceback.print_exc(file=sys.stderr)
            fail(request_id, str(error), retryable=False)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
