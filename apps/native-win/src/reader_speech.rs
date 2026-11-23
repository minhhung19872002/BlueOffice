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
    /// The installed voices (BlueDoc Read Aloud › Settings, round 17).
    tokens: Vec<windows::Win32::Media::Speech::ISpObjectToken>,
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

    /// The installed voices' names and the index of the one in use (round 17).
    pub fn voices(&mut self) -> (Vec<String>, usize) {
        use windows::Win32::Media::Speech::{ISpObjectTokenCategory, SpObjectTokenCategory};
        use windows::Win32::System::Com::CoTaskMemFree;
        let Ok(v) = self.voice().cloned() else {
            return (Vec::new(), 0);
        };
        let id = wide("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices");
        let mut names = Vec::new();
        let mut tokens = Vec::new();
        let mut current = 0;
        // SAFETY: COM is initialised (the voice exists); strings returned by SAPI are copied
        // and freed with CoTaskMemFree.
        unsafe {
            let Ok(cat) = CoCreateInstance::<_, ISpObjectTokenCategory>(
                &SpObjectTokenCategory,
                None,
                CLSCTX_ALL,
            ) else {
                return (Vec::new(), 0);
            };
            if cat.SetId(PCWSTR(id.as_ptr()), false).is_err() {
                return (Vec::new(), 0);
            }
            let Ok(list) = cat.EnumTokens(PCWSTR::null(), PCWSTR::null()) else {
                return (Vec::new(), 0);
            };
            let mut n = 0u32;
            let _ = list.GetCount(&raw mut n);
            let now = v.GetVoice().ok().and_then(|t| {
                let p = t.GetId().ok()?;
                let s = p.to_string().ok();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            });
            for i in 0..n {
                let Ok(t) = list.Item(i) else { continue };
                let name = t.GetStringValue(PCWSTR::null()).ok().and_then(|p| {
                    let s = p.to_string().ok();
                    CoTaskMemFree(Some(p.0 as *const _));
                    s
                });
                let tid = t.GetId().ok().and_then(|p| {
                    let s = p.to_string().ok();
                    CoTaskMemFree(Some(p.0 as *const _));
                    s
                });
                if tid.is_some() && tid == now {
                    current = names.len();
                }
                names.push(name.unwrap_or_else(|| format!("Voice {}", i + 1)));
                tokens.push(t);
            }
        }
        self.tokens = tokens;
        (names, current)
    }

    /// Speak with voice `i` of [`Speech::voices`] from now on.
    pub fn set_voice(&mut self, i: usize) {
        let Some(t) = self.tokens.get(i).cloned() else {
            return;
        };
        if let Ok(v) = self.voice() {
            // SAFETY: plain call on a live voice with a token it listed.
            unsafe {
                let _ = v.SetVoice(&t);
            }
        }
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
