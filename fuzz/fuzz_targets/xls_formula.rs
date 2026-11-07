#![no_main]
use bo_binary::xls_formula::{formula_to_text_with, Dialect, FormulaCtx};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let ctx = FormulaCtx {
        sheet_names: vec!["A".into(), "B".into()],
        xti: vec![(0, 0, 1)],
        ..FormulaCtx::default()
    };
    let split = data.first().map_or(0, |&b| b as usize).min(data.len());
    let (rgce, rgcb) = data.split_at(split);
    let _ = formula_to_text_with(Dialect::Biff8, rgce, rgcb, &ctx, (1, 1));
    let _ = formula_to_text_with(Dialect::Biff12, rgce, rgcb, &ctx, (1, 1));
});
