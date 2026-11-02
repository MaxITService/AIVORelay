//! Finds and temporarily takes a Bluetooth headset on macOS.
//! Playing output moves a headset to this Mac; recording from its mic alone
//! does not.

/*
CoreAudio lookup adapted from handy-recorder 0.1.0:
MIT License

Copyright (c) 2026 handy-computer

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    ffi::c_void,
    ptr::{NonNull, null},
};

use objc2_core_audio::{
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectPropertyScope, AudioObjectPropertySelector,
    kAudioDevicePropertyDeviceUID, kAudioDevicePropertyRelatedDevices, kAudioDevicePropertyStreams,
    kAudioDevicePropertyTransportType, kAudioDeviceTransportTypeBluetooth,
    kAudioDeviceTransportTypeBluetoothLE, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeInput,
    kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
};
use objc2_core_foundation::{CFRetained, CFString};

/// Returns None rather than guessing if CPAL's name cannot identify one device.
/// The returned stream must be created, played and dropped on the capture worker.
pub(super) fn take_headset(input: &cpal::Device) -> Option<cpal::Stream> {
    match try_take_headset(input) {
        Ok(stream) => stream,
        Err(error) => {
            log::warn!("AirPods mode could not take the headset: {error}");
            None
        }
    }
}

fn try_take_headset(input: &cpal::Device) -> Result<Option<cpal::Stream>, String> {
    let input_name = input.name().map_err(|error| error.to_string())?;
    let Some(output_name) = bluetooth_output_name(&input_name) else { return Ok(None) };
    let host = crate::audio_toolkit::get_cpal_host();
    let mut outputs = host.output_devices().map_err(|error| error.to_string())?
        .filter(|device| device.name().ok().as_deref() == Some(output_name.as_str()));
    let Some(output) = outputs.next() else { return Ok(None) };
    if outputs.next().is_some() {
        log::warn!("AirPods mode skipped an ambiguous output device name");
        return Ok(None);
    }
    let config = output.default_output_config().map_err(|error| error.to_string())?;
    // Zero bytes are silence for signed PCM and IEEE floats, but not unsigned PCM.
    if !matches!(config.sample_format(), cpal::SampleFormat::F32 | cpal::SampleFormat::F64
        | cpal::SampleFormat::I8 | cpal::SampleFormat::I16 | cpal::SampleFormat::I32 | cpal::SampleFormat::I64)
    {
        return Err("Unsupported silent output sample format".to_string());
    }
    let stream = output.build_output_stream_raw(
        &config.config(), config.sample_format(),
        |data, _| data.bytes_mut().fill(0),
        |error| log::warn!("AirPods silent output stream failed: {error}"), None,
    ).map_err(|error| error.to_string())?;
    stream.play().map_err(|error| error.to_string())?;
    log::debug!("AirPods mode started silent headset playback");
    Ok(Some(stream))
}

fn bluetooth_output_name(input_name: &str) -> Option<String> {
    let devices = devices();
    let mut inputs = devices.iter().copied().filter(|&device| {
        string_property(device, kAudioObjectPropertyName).as_deref() == Some(input_name)
            && !ids(device, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput).is_empty()
    });
    let input = inputs.next()?;
    if inputs.next().is_some() {
        log::warn!("AirPods mode skipped an ambiguous input device name");
        return None;
    }
    let input_uid = uid(input)?;
    if !is_bluetooth(input) {
        log::debug!("take_headset: {input_uid:?} is not a Bluetooth device");
        return None;
    }
    let has_output = |d: AudioObjectID| {
        is_bluetooth(d)
            && !ids(
                d,
                kAudioDevicePropertyStreams,
                kAudioObjectPropertyScopeOutput,
            )
            .is_empty()
    };
    // The documented link.
    let related = ids(
        input,
        kAudioDevicePropertyRelatedDevices,
        kAudioObjectPropertyScopeGlobal,
    );
    if let Some(output) = related
        .iter()
        .copied()
        .find(|&d| has_output(d))
    {
        return string_property(output, kAudioObjectPropertyName);
    }
    // Fallback: both UIDs are the Bluetooth address plus ":input"/":output".
    let output = input_uid.rsplit_once(':').and_then(|(address, _)| {
        devices
            .iter()
            .copied()
            .filter(|&d| has_output(d))
            .find(|&d| uid(d).is_some_and(|u| u.rsplit_once(':').is_some_and(|(a, _)| a == address)))
    });
    if output.is_none() {
        log::debug!("take_headset: found no output device for {input_uid:?}");
    }
    output.and_then(|device| string_property(device, kAudioObjectPropertyName))
}

fn is_bluetooth(device: AudioObjectID) -> bool {
    scalar(device, kAudioDevicePropertyTransportType).is_some_and(|t| {
        t == kAudioDeviceTransportTypeBluetooth || t == kAudioDeviceTransportTypeBluetoothLE
    })
}

fn address(
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn devices() -> Vec<AudioObjectID> {
    ids(
        kAudioObjectSystemObject as AudioObjectID,
        kAudioHardwarePropertyDevices,
        kAudioObjectPropertyScopeGlobal,
    )
}

/// A property holding an array of object IDs.
fn ids(
    object: AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Vec<AudioObjectID> {
    let address = address(selector, scope);
    let mut size = 0u32;
    // SAFETY: the address and size outlive the call.
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            object,
            NonNull::from(&address),
            0,
            null(),
            NonNull::from(&mut size),
        )
    };
    if status != 0 || size == 0 || size as usize % size_of::<AudioObjectID>() != 0 {
        return Vec::new();
    }
    let mut out = vec![0 as AudioObjectID; size as usize / size_of::<AudioObjectID>()];
    let mut size = (out.len() * size_of::<AudioObjectID>()) as u32;
    // SAFETY: `out` has room for `size` bytes, and CoreAudio writes at most
    // that many, updating `size` to what it wrote.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&address),
            0,
            null(),
            NonNull::from(&mut size),
            NonNull::new(out.as_mut_ptr()).unwrap().cast::<c_void>(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    out.truncate(size as usize / size_of::<AudioObjectID>());
    out
}

fn scalar(object: AudioObjectID, selector: AudioObjectPropertySelector) -> Option<u32> {
    let address = address(selector, kAudioObjectPropertyScopeGlobal);
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` has room for the `size` bytes CoreAudio may write.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&address),
            0,
            null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast(),
        )
    };
    (status == 0 && size == size_of::<u32>() as u32).then_some(value)
}

fn uid(device: AudioObjectID) -> Option<String> {
    string_property(device, kAudioDevicePropertyDeviceUID)
}

fn string_property(device: AudioObjectID, selector: AudioObjectPropertySelector) -> Option<String> {
    let address = address(
        selector,
        kAudioObjectPropertyScopeGlobal,
    );
    let mut uid: *mut CFString = std::ptr::null_mut();
    let mut size = size_of::<*mut CFString>() as u32;
    // SAFETY: the UID property is a CFString pointer, returned under the
    // create rule (+1), which `CFRetained::from_raw` takes over.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            NonNull::from(&address),
            0,
            null(),
            NonNull::from(&mut size),
            NonNull::from(&mut uid).cast(),
        )
    };
    if status != 0 {
        return None;
    }
    let uid = NonNull::new(uid)?;
    // SAFETY: see above; the pointer is non-null and owned.
    Some(unsafe { CFRetained::from_raw(uid) }.to_string())
}
