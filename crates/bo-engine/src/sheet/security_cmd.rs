//! Security commands of BlueSheet (CORE-SEC-001): final mark, read-only
//! recommendation and password to modify (`fileSharing`, ECMA-376 Part 1,
//! §18.2.12), labels, and protected ranges (§18.3.1.71).

use super::cmd::Recalc;
use super::protect;
use crate::security::{FileSecurity, SecurityCommand, SecurityInfo, READ_ONLY};
use crate::security_cmd::common_command;
use crate::{EngineError, Result, SheetEngine};
use serde_json::{json, Value};

impl SheetEngine {
    /// `Err(read only)` while the workbook is open read-only.
    pub(crate) fn writable(&self) -> Result<()> {
        if self.sec.read_only {
            Err(EngineError::Sheet(READ_ONLY.into()))
        } else {
            Ok(())
        }
    }

    /// Protected ranges of `sheet` unlocked in this session (range names).
    pub(crate) fn open_ranges(&self, sheet: usize) -> Vec<String> {
        let Some(ws) = self.wb.as_ref().and_then(|wb| wb.sheets.get(sheet)) else {
            return Vec::new();
        };
        self.open_ranges
            .iter()
            .filter(|(s, _)| *s == ws.name)
            .map(|(_, n)| n.clone())
            .collect()
    }

    /// Reads the security metadata of the workbook just opened.
    pub(crate) fn open_security(&mut self, bytes: &[u8]) {
        let mut sec = FileSecurity::read(bytes);
        let fs = self
            .wb
            .as_ref()
            .map(protect::file_sharing_info)
            .unwrap_or_default();
        sec.read_only = sec.opens_read_only(fs.read_only_recommended, fs.has_password);
        self.sec = sec;
    }

    /// Protection state of the workbook.
    pub fn security_info(&self) -> SecurityInfo {
        let fs = self
            .wb
            .as_ref()
            .map(protect::file_sharing_info)
            .unwrap_or_default();
        SecurityInfo {
            marked_final: self.sec.marked_final(),
            read_only_recommended: fs.read_only_recommended,
            write_password: fs.has_password,
            read_only: self.sec.read_only,
            label: self.sec.label(),
            foreign_labels: self.sec.foreign().to_vec(),
            protection: None,
            regions: Vec::new(),
            region_rects: Vec::new(),
            show_regions: false,
            identity: self.identity.clone(),
        }
    }

    /// The workbook changed security metadata since it was saved.
    pub fn security_dirty(&self) -> bool {
        self.sec.dirty()
    }

    /// Call after the saved bytes are stored.
    pub fn mark_security_saved(&mut self) {
        self.sec.mark_saved();
    }

    /// Runs a security command; returns `{ security, ranges?, error? }`.
    pub fn sheet_security(&mut self, cmd: SecurityCommand) -> Result<Value> {
        if self.wb.is_none() {
            return Err(EngineError::NoDocument);
        }
        let mut error: Option<String> = None;
        let mut ranges = None;
        if self.sec.read_only && !cmd.is_view() {
            error = Some(READ_ONLY.into());
        } else {
            let attrs = self.wb.as_ref().and_then(|wb| {
                protect::find(&wb.unknown, "fileSharing")
                    .map(|i| protect::parse_attrs(&wb.unknown[i].xml))
            });
            match common_command(
                &mut self.sec,
                &mut self.identity,
                &cmd,
                attrs.as_ref().map(|a| (a, "reservationPassword")),
            ) {
                Err(e) => error = Some(e),
                Ok(true) => {}
                Ok(false) => {
                    if let Err(e) = self.sheet_only(cmd, &mut ranges) {
                        error = Some(match e {
                            EngineError::Sheet(m) | EngineError::BadCommand(m) => m,
                            other => other.to_string(),
                        });
                    }
                }
            }
        }
        let mut out = json!({ "security": self.security_info() });
        if let Some(r) = ranges {
            out["ranges"] = r;
        }
        if let Some(e) = error {
            out["error"] = json!(e);
        }
        Ok(out)
    }

    fn sheet_only(&mut self, cmd: SecurityCommand, ranges: &mut Option<Value>) -> Result<()> {
        use SecurityCommand as S;
        match cmd {
            S::SetReadOnlyRecommended { on } => self.edit(0, Recalc::No, |wb, _| {
                protect::set_file_sharing(wb, Some(on), None, None)
            })?,
            S::SetWritePassword { password } => {
                let user = self.identity.first().cloned();
                self.edit(0, Recalc::No, |wb, _| {
                    protect::set_file_sharing(wb, None, Some(&password), user.as_deref())
                })?;
                self.sec.write_unlocked = true;
            }
            S::ProtectedRanges { sheet } => *ranges = Some(self.ranges_json(sheet)?),
            S::AddProtectedRange {
                sheet,
                name,
                sqref,
                password,
            } => {
                self.unprotected(sheet)?;
                self.edit(sheet, Recalc::No, |wb, _| {
                    protect::add_protected_range(&mut wb.sheets[sheet], &name, &sqref, &password)
                })?;
                *ranges = Some(self.ranges_json(sheet)?);
            }
            S::RemoveProtectedRange { sheet, name } => {
                self.unprotected(sheet)?;
                let found = self.edit(sheet, Recalc::No, |wb, _| {
                    Ok(protect::remove_protected_range(
                        &mut wb.sheets[sheet],
                        &name,
                    ))
                })?;
                if !found {
                    return Err(EngineError::BadCommand(format!(
                        "no protected range {name}"
                    )));
                }
                *ranges = Some(self.ranges_json(sheet)?);
            }
            S::UnlockProtectedRange {
                sheet,
                name,
                password,
            } => {
                let ws = self.sheet(sheet)?;
                if !protect::verify_range_password(ws, &name, &password)? {
                    return Err(EngineError::Sheet(
                        protect::WRONG_PROTECTION_PASSWORD.into(),
                    ));
                }
                let key = (ws.name.clone(), name);
                if !self.open_ranges.contains(&key) {
                    self.open_ranges.push(key);
                }
                *ranges = Some(self.ranges_json(sheet)?);
            }
            S::Security
            | S::SetIdentity { .. }
            | S::SetReadOnly { .. }
            | S::UnlockWrite { .. }
            | S::MarkFinal { .. }
            | S::SetLabel { .. } => {}
            S::ProtectDocument { .. }
            | S::UnprotectDocument { .. }
            | S::AddEditableRegion { .. }
            | S::RemoveEditableRegion { .. }
            | S::NextEditableRegion { .. }
            | S::ShowEditableRegions { .. } => {
                return Err(EngineError::BadCommand("not a document".into()));
            }
        }
        Ok(())
    }

    /// Ranges are defined while the sheet is not protected.
    fn unprotected(&self, sheet: usize) -> Result<()> {
        if protect::sheet_info(self.sheet(sheet)?).protected {
            Err(EngineError::Sheet(protect::SHEET_PROTECTED.into()))
        } else {
            Ok(())
        }
    }

    fn ranges_json(&self, sheet: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let open = self.open_ranges(sheet);
        Ok(Value::Array(
            protect::protected_ranges(ws)
                .into_iter()
                .map(|r| {
                    let unlocked =
                        !r.has_password || open.iter().any(|n| n.eq_ignore_ascii_case(&r.name));
                    let mut v = json!(r);
                    v["unlocked"] = json!(unlocked);
                    v
                })
                .collect(),
        ))
    }
}
