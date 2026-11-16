//! Read aloud for BlueReader through the Windows speech voice (SAPI `ISpVoice`), created only
//! when the user asks to read (Win32 calls confined to this child module, like win32.rs).
use windows::core::PCWSTR;
use windows::Win32::Media::Speech::{
    ISpVoice, SpVoice, SPF_ASYNC, SPF_IS_NOT_XML, SPF_PURGEBEFORESPEAK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};

/// The speech voice, made on first use.
#[derive(Default)]
pub struct Speech {
    voice: Option<ISpVoice>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Speech {
    fn voice(&mut self) -> Result<&ISpVoice, String> {
        if self.voice.is_none() {
            // SAFETY: COM is initialised on this (the window's) thread before the voice is made;
            // a second initialisation only answers S_FALSE.
            let v: ISpVoice = unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                CoCreateInstance(&SpVoice, None, CLSCTX_ALL).map_err(|e| e.to_string())?
            };
            self.voice = Some(v);
        }
        self.voice
            .as_ref()
            .ok_or_else(|| "no speech voice".to_owned())
    }

    /// Speak `text` at `rate` (−10…10), replacing what is being read.
    pub fn speak(&mut self, text: &str, rate: i32) -> Result<(), String> {
        let v = self.voice()?;
        let w = wide(text);
        // SAFETY: the text buffer lives for the call; SAPI copies it for asynchronous speech.
        unsafe {
            v.SetRate(rate.clamp(-10, 10)).map_err(|e| e.to_string())?;
            let _ = v.Resume();
            v.Speak(
                PCWSTR(w.as_ptr()),
                (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0 | SPF_IS_NOT_XML.0) as u32,
                None,
            )
            .map_err(|e| e.to_string())
        }
    }

    /// The word being spoken: its UTF-16 offset in the text and its length (BlueReader round
    /// 11 highlights it on the page); `None` when nothing is being read.
    pub fn word(&self) -> Option<(usize, usize)> {
        let v = self.voice.as_ref()?;
        let mut st = windows::Win32::Media::Speech::SPVOICESTATUS::default();
        // SAFETY: the status is a plain struct written by the call; no bookmark is asked for
        // (a null pointer), so nothing is allocated for us to free.
        unsafe {
            v.GetStatus(&raw mut st, std::ptr::null_mut()).ok()?;
        }
        // SPRS_IS_SPEAKING (2): running.
        (st.dwRunningState & 2 != 0)
            .then_some((st.ulInputWordPos as usize, st.ulInputWordLen as usize))
    }

    /// 0 stop, 1 pause, 2 resume, 10 ± n: rate n.
    pub fn control(&mut self, what: i32) {
        let Some(v) = self.voice.as_ref() else {
            return;
        };
        // SAFETY: plain calls on a live voice; an empty purge stops speaking.
        unsafe {
            match what {
                0 => {
                    let empty = wide("");
                    let _ = v.Resume();
                    let _ = v.Speak(
                        PCWSTR(empty.as_ptr()),
                        (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0) as u32,
                        None,
                    );
                }
                1 => {
                    let _ = v.Pause();
                }
                2 => {
                    let _ = v.Resume();
                }
                r => {
                    let _ = v.SetRate((r - 10).clamp(-10, 10));
                }
            }
        }
    }
}
