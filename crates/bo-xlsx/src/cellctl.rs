//! Checkbox cell controls ([MS-XLSX] §2.3.9 Feature Property Bag).
//!
//! A cell format (`cellXfs/xf`, ECMA-376 Part 1, §18.8.45) shows a checkbox when
//! its `extLst` holds `xfpb:xfComplement i="n"` (ext `{C7286773-…}`, [MS-XLSX]
//! §2.2.4.9, §2.6.238) and, in the Feature Property Bag part (§2.1.22), the
//! `n`-th entry of `MappedFeaturePropertyBags` of the `XFComplements` bag
//! (§2.3.9.1) is an `XFComplement` bag (§2.3.9.2) whose `XFControls` bag
//! (§2.3.9.3) has a `CellControl` of type `Checkbox` (§2.3.9.4).
//!
//! Reading resolves that chain into [`CellXf::checkbox`]. Writing keeps the
//! part byte-for-byte when a checkbox complement already exists in it; otherwise
//! the three bags are appended (or a new part is created) and the new
//! complement index is written on every checkbox format.

use crate::xmlpatch::{children, index, El, Patch};
use bo_model::sheet::{CellXf, Workbook};

/// Feature property bag relationship (workbook → part, [MS-XLSX] §2.1.22).
pub const REL_FPB: &str =
    "http://schemas.microsoft.com/office/2022/11/relationships/FeaturePropertyBag";
/// Feature property bag content type ([MS-XLSX] §2.1.22).
pub const CT_FPB: &str = "application/vnd.ms-excel.featurepropertybag+xml";
/// Namespace of `FeaturePropertyBags` and `xfComplement` ([MS-XLSX] §2.4.98).
pub const NS_FPB: &str =
    "http://schemas.microsoft.com/office/spreadsheetml/2022/featurepropertybag";
/// Ext URI of `xfComplement` in `xf/extLst` ([MS-XLSX] §2.2.4.9 Styles).
pub const EXT_XF_COMPLEMENT: &str = "{C7286773-470A-42A8-94C5-96B5CB345126}";

/// For each `MappedFeaturePropertyBags` index: does it lead to a checkbox?
pub(crate) fn checkbox_complements(data: &[u8], part: &str) -> Vec<bool> {
    let Some(els) = index(data, part) else {
        return Vec::new();
    };
    let bags = bags(&els);
    let Some(&complements) = bags
        .iter()
        .find(|&&b| els[b].attr("type").as_deref() == Some("XFComplements"))
    else {
        return Vec::new();
    };
    mapped(&els, complements)
        .into_iter()
        .map(|id| is_checkbox_complement(&els, &bags, id))
        .collect()
}

/// Element indices of the `bag` children of the root, in order (= bag ids).
fn bags(els: &[El]) -> Vec<usize> {
    children(els, 0)
        .filter(|(_, e)| e.local == "bag")
        .map(|(i, _)| i)
        .collect()
}

/// The `bagId` values of the `a k="MappedFeaturePropertyBags"` array of a bag.
fn mapped(els: &[El], bag: usize) -> Vec<u32> {
    children(els, bag)
        .filter(|(_, e)| {
            e.local == "a" && e.attr("k").as_deref() == Some("MappedFeaturePropertyBags")
        })
        .flat_map(|(a, _)| {
            children(els, a)
                .filter(|(_, e)| e.local == "bagId")
                .filter_map(|(_, e)| e.text.trim().parse().ok())
                .collect::<Vec<u32>>()
        })
        .collect()
}

/// Value of the `bagId k="key"` property of bag element `bag`.
fn bag_ref(els: &[El], bag: usize, key: &str) -> Option<u32> {
    children(els, bag)
        .find(|(_, e)| e.local == "bagId" && e.attr("k").as_deref() == Some(key))
        .and_then(|(_, e)| e.text.trim().parse().ok())
}

fn bag_type(els: &[El], bags: &[usize], id: u32) -> Option<(usize, String)> {
    let i = *bags.get(id as usize)?;
    Some((i, els[i].attr("type").unwrap_or_default()))
}

fn is_checkbox_complement(els: &[El], bags: &[usize], id: u32) -> bool {
    let step = |id: u32, want: &str, key: &str| -> Option<u32> {
        let (i, t) = bag_type(els, bags, id)?;
        (t == want).then(|| bag_ref(els, i, key)).flatten()
    };
    step(id, "XFComplement", "XFControls")
        .and_then(|c| step(c, "XFControls", "CellControl"))
        .and_then(|cb| bag_type(els, bags, cb))
        .is_some_and(|(_, t)| t == "Checkbox")
}

/// After reading: mark the formats whose complement is a checkbox.
pub(crate) fn resolve(wb: &mut Workbook) {
    let Some(part) = fpb_part(wb) else { return };
    let Some(data) = wb
        .opaque
        .parts
        .iter()
        .find(|p| p.name == part)
        .map(|p| &p.data)
    else {
        return;
    };
    let flags = checkbox_complements(data, &part);
    for xf in &mut wb.styles.cell_xfs {
        if let Some(i) = xf.complement {
            if flags.get(i as usize) == Some(&true) {
                xf.checkbox = true;
            }
        }
    }
}

/// Absolute name of the feature property bag part, if the workbook has one.
pub(crate) fn fpb_part(wb: &Workbook) -> Option<String> {
    wb.opaque
        .workbook_rels
        .iter()
        .find(|r| !r.external && r.rel_type.ends_with("/FeaturePropertyBag"))
        .map(|r| r.target.clone())
}

/// What the writer must do for checkbox formats.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    /// Complement index to write on checkbox formats.
    pub checkbox: Option<u32>,
    /// Complement indices (as read) that lead to a checkbox.
    pub checkbox_ids: Vec<u32>,
    /// New content of the existing part (`Some` when it was patched).
    pub patched: Option<Vec<u8>>,
    /// New part to create (no part in the source workbook).
    pub created: Option<Vec<u8>>,
}

/// Plans the feature property bag part for the formats of `wb`.
pub(crate) fn plan(wb: &Workbook) -> Plan {
    let any = wb.styles.cell_xfs.iter().any(|x| x.checkbox);
    let existing = fpb_part(wb).and_then(|n| {
        wb.opaque
            .parts
            .iter()
            .find(|p| p.name == n)
            .map(|p| (n, p.data.clone()))
    });
    let mut plan = Plan::default();
    if let Some((name, data)) = &existing {
        let flags = checkbox_complements(data, name);
        plan.checkbox_ids = flags
            .iter()
            .enumerate()
            .filter(|(_, f)| **f)
            .map(|(i, _)| i as u32)
            .collect();
        if !any {
            return plan;
        }
        if let Some(&first) = plan.checkbox_ids.first() {
            plan.checkbox = Some(first);
            return plan;
        }
        if let Some((patched, id)) = append_checkbox(data, name) {
            plan.checkbox = Some(id);
            plan.patched = Some(patched);
            return plan;
        }
        // Unreadable part: leave it, write the formats without the control.
        return plan;
    }
    if any {
        plan.checkbox = Some(0);
        plan.created = Some(new_part().into_bytes());
    }
    plan
}

/// A new part with one checkbox complement (index 0).
fn new_part() -> String {
    // [MS-XLSX] §2.3.9: bag 0 Checkbox, 1 XFControls → 0, 2 XFComplement → 1,
    // 3 XFComplements mapping complement 0 → bag 2 (a bagId value must be lower than
    // the referencing bag's position, §2.6.230).
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <FeaturePropertyBags xmlns=\"{NS_FPB}\" count=\"4\">{}</FeaturePropertyBags>",
        bag_xml("", 0, true)
    )
}

/// The three bags of a checkbox complement starting at bag id `first`, plus the
/// `XFComplements` bag when `with_complements`.
fn bag_xml(prefix: &str, first: u32, with_complements: bool) -> String {
    let q = |l: &str| {
        if prefix.is_empty() {
            l.to_owned()
        } else {
            format!("{prefix}:{l}")
        }
    };
    let (bag, bag_id, a) = (q("bag"), q("bagId"), q("a"));
    let mut s = format!(
        "<{bag} type=\"Checkbox\"/>\
         <{bag} type=\"XFControls\"><{bag_id} k=\"CellControl\">{first}</{bag_id}></{bag}>\
         <{bag} type=\"XFComplement\"><{bag_id} k=\"XFControls\">{}</{bag_id}></{bag}>",
        first + 1
    );
    if with_complements {
        s.push_str(&format!(
            "<{bag} type=\"XFComplements\"><{a} k=\"MappedFeaturePropertyBags\">\
             <{bag_id}>{}</{bag_id}></{a}></{bag}>",
            first + 2
        ));
    }
    s
}

/// Appends a checkbox complement to an existing part; returns the new part and
/// the complement index.
fn append_checkbox(data: &[u8], part: &str) -> Option<(Vec<u8>, u32)> {
    let els = index(data, part)?;
    let root = els.first()?;
    let prefix = root.prefix().to_owned();
    let bags = bags(&els);
    let first = bags.len() as u32;
    let complements = bags
        .iter()
        .copied()
        .find(|&b| els[b].attr("type").as_deref() == Some("XFComplements"));
    let mut p = Patch::default();
    let new_bags = bag_xml(&prefix, first, complements.is_none());
    // Bags precede the root's `extLst` (§2.6.234 CT_FeaturePropertyBags sequence).
    match children(&els, 0).find(|(_, e)| e.local == "extLst") {
        Some((_, ext)) => p.insert_before(ext, &new_bags),
        None => p.append(0, &new_bags),
    }
    let added = if complements.is_some() { 3 } else { 4 };
    if root.attr("count").is_some() {
        p.set_attr(0, "count", &(first + added).to_string());
    }
    let id = match complements {
        Some(c) => {
            let n = mapped(&els, c).len() as u32;
            let bag_id = root.q("bagId");
            let item = format!("<{bag_id}>{}</{bag_id}>", first + 2);
            match children(&els, c).find(|(_, e)| {
                e.local == "a" && e.attr("k").as_deref() == Some("MappedFeaturePropertyBags")
            }) {
                Some((a, _)) => p.append(a, &item),
                None => p.append(
                    c,
                    &format!(
                        "<{a} k=\"MappedFeaturePropertyBags\">{item}</{a}>",
                        a = root.q("a")
                    ),
                ),
            }
            n
        }
        None => 0,
    };
    Some((p.apply(data, &els), id))
}

/// The complement index to write on `xf` under `plan`.
pub(crate) fn complement_of(xf: &CellXf, plan: &Plan) -> Option<u32> {
    if xf.checkbox {
        return plan.checkbox.or(xf.complement);
    }
    // A format that no longer shows a checkbox drops a checkbox complement.
    xf.complement.filter(|i| !plan.checkbox_ids.contains(i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_part_maps_complement_zero_to_a_checkbox() {
        let p = new_part();
        assert_eq!(checkbox_complements(p.as_bytes(), "/x.xml"), vec![true]);
    }

    #[test]
    fn appends_to_a_part_without_checkbox() {
        let src = format!(
            "<x:FeaturePropertyBags xmlns:x=\"{NS_FPB}\" count=\"2\"><x:bag type=\"Other\"/>\
             <x:bag type=\"XFComplements\"><x:a k=\"MappedFeaturePropertyBags\"><x:bagId>0</x:bagId></x:a></x:bag>\
             </x:FeaturePropertyBags>"
        );
        let (out, id) = append_checkbox(src.as_bytes(), "/x.xml").unwrap();
        // Bags 2, 3, 4 are appended; complement 1 → bag 4 → XFControls 3 → Checkbox 2.
        assert_eq!(id, 1);
        assert_eq!(
            checkbox_complements(&out, "/x.xml"),
            vec![false, true],
            "{}",
            String::from_utf8_lossy(&out)
        );
        assert!(String::from_utf8_lossy(&out).contains("count=\"5\""));
    }
}
