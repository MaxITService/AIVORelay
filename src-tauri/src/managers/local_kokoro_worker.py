"""AivoRelay managed Kokoro/sherpa-onnx worker.

The worker keeps the native sherpa-onnx engine resident and exchanges only
versioned JSON Lines with AivoRelay. Third-party output is redirected to
stderr so it cannot corrupt protocol messages.
"""

from __future__ import annotations

import argparse
import array
import base64
import contextlib
import json
import math
import os
from pathlib import Path
import sys
import traceback
import wave


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


def write_pcm16_wave(path: Path, samples, sample_rate: int) -> int:
    pcm = array.array("h")
    for value in samples:
        sample = float(value)
        if not math.isfinite(sample):
            raise RuntimeError("Kokoro returned non-finite audio")
        pcm.append(round(max(-1.0, min(1.0, sample)) * 32767.0))
    if not pcm:
        raise RuntimeError("Kokoro returned no audio samples")
    if sys.byteorder != "little":
        pcm.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(sample_rate)
        output.writeframes(pcm.tobytes())
    return len(pcm)


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--model-root", required=True)
    parser.add_argument("--output-root", required=True)
    parser.add_argument("--threads", type=int, default=2)
    args = parser.parse_args()

    model_root = Path(args.model_root).resolve()
    output_root = Path(args.output_root).resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    for stale_output in output_root.glob("request-*.wav"):
        try:
            stale_output.unlink()
        except OSError:
            pass
    required = (
        "model.int8.onnx",
        "voices.bin",
        "tokens.txt",
        "espeak-ng-data",
        "lexicon-us-en.txt",
        "lexicon-zh.txt",
    )
    if not model_root.is_dir() or any(
        not (model_root / name).exists() for name in required
    ):
        fail(None, "The managed Kokoro model directory is incomplete")
        return 2

    # Synthesis must remain offline. Installation is the only network phase.
    os.environ["HF_HUB_OFFLINE"] = "1"

    try:
        with contextlib.redirect_stdout(sys.stderr):
            import sherpa_onnx

            config = sherpa_onnx.OfflineTtsConfig(
                model=sherpa_onnx.OfflineTtsModelConfig(
                    kokoro=sherpa_onnx.OfflineTtsKokoroModelConfig(
                        model=str(model_root / "model.int8.onnx"),
                        voices=str(model_root / "voices.bin"),
                        tokens=str(model_root / "tokens.txt"),
                        data_dir=str(model_root / "espeak-ng-data"),
                        lexicon=(
                            f"{model_root / 'lexicon-us-en.txt'},"
                            f"{model_root / 'lexicon-zh.txt'}"
                        ),
                    ),
                    num_threads=max(1, min(8, args.threads)),
                    debug=False,
                    provider="cpu",
                )
            )
            if not config.validate():
                raise RuntimeError("sherpa-onnx rejected the pinned Kokoro configuration")
            model = sherpa_onnx.OfflineTts(config)
    except Exception as error:
        traceback.print_exc(file=sys.stderr)
        fail(None, f"Failed to load Kokoro: {error}")
        return 3

    emit(
        {
            "type": "ready",
            "protocol": PROTOCOL_VERSION,
            "sample_rate": int(model.sample_rate),
            "num_speakers": int(model.num_speakers),
            "runtime_version": getattr(sherpa_onnx, "__version__", "1.13.4"),
        }
    )

    for raw_line in sys.stdin:
        request_id: str | None = None
        output_path: Path | None = None
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
            sid = int(request.get("sid", -1))
            speed = float(request.get("speed", 1.0))
            if not text:
                raise ValueError("Synthesis text must not be empty")
            if not 0 <= sid < 103:
                raise ValueError("Kokoro speaker ID must be between 0 and 102")
            if not 0.5 <= speed <= 2.0:
                raise ValueError("Kokoro speed must be between 0.5 and 2.0")

            output_path = contained_path(output_root, request["output_path"])
            output_path.parent.mkdir(parents=True, exist_ok=True)
            if output_path.exists():
                output_path.unlink()

            with contextlib.redirect_stdout(sys.stderr):
                audio = model.generate(text=text, sid=sid, speed=speed)
            sample_rate = int(audio.sample_rate)
            if sample_rate != 24000:
                raise RuntimeError("Kokoro returned an unexpected sample rate")
            sample_count = write_pcm16_wave(output_path, audio.samples, sample_rate)
            emit(
                {
                    "type": "result",
                    "protocol": PROTOCOL_VERSION,
                    "id": request_id,
                    "output_path": str(output_path),
                    "sample_rate": sample_rate,
                    "samples": sample_count,
                }
            )
        except Exception as error:
            if output_path is not None:
                try:
                    output_path.unlink(missing_ok=True)
                except OSError:
                    pass
            traceback.print_exc(file=sys.stderr)
            fail(request_id, str(error), retryable=False)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
