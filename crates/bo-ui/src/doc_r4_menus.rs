//! Menus, labels, icons and gallery tiles of BlueDoc's round-4 commands (Insert, Layout and
//! View tabs laid out like the reference's): command ids, their entries in the reference's
//! order and casing, which are unavailable (dimmed as the reference shows features BlueDoc
//! does not have), and BlueOffice's own drawings for the icons and gallery tiles.
use crate::catalog::Entry;
use crate::icons::Icons;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Gallery drop-downs of round 4 (opened as tile menus).
pub(crate) const GALLERIES: [&str; 13] = [
    "insert.coverPage",
    "insert.header",
    "insert.footer",
    "insert.textBox",
    "insert.equation",
    "insert.shapes",
    "insert.smartArt",
    "insert.chart",
    "insert.wordArt",
    "layout.margins",
    "layout.orientation",
    "layout.size",
    "layout.columns",
];

/// Menu buttons of round 4 (plain menus).
pub(crate) const MENUS: [&str; 8] = [
    "insert.pageNumber",
    "insert.quickParts",
    "insert.object",
    "insert.signatureMenu",
    "layout.position",
    "layout.wrapText",
    "layout.align",
    "layout.rotate",
];

/// Buttons of round 4 that are not menus.
pub(crate) const BUTTONS: [&str; 22] = [
    "insert.blankPage",
    "insert.icons",
    "insert.models3d",
    "insert.screenshot",
    "insert.dateTime",
    "layout.indentLeft",
    "layout.indentRight",
    "layout.spaceBefore",
    "layout.spaceAfter",
    "layout.bringForward",
    "layout.sendBackward",
    "layout.selectionPane",
    "layout.group",
    "view.immersiveReader",
    "view.focus",
    "view.gridlines",
    "view.zoomDialog",
    "view.multiplePages",
    "view.newWindow",
    "view.arrangeAll",
    "view.split",
    "view.sideBySide",
];

/// More buttons of round 4 (View tab).
pub(crate) const MORE_BUTTONS: [&str; 3] = ["view.macros", "view.vertical", "view.sideToSide"];

/// Features of the reference BlueDoc does not have: shown dimmed, as the brief says
/// ("-free"), so the tab keeps the reference's layout.
pub(crate) const UNAVAILABLE: [&str; 2] = ["insert.equation.ink", "view.newWindow"];

/// Every `&'static` id of round 4.
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    GALLERIES
        .iter()
        .chain(MENUS.iter())
        .chain(BUTTONS.iter())
        .chain(MORE_BUTTONS.iter())
        .find(|m| **m == id)
        .copied()
}

/// Whether round-4 command `id` is enabled (`None`: not a round-4 id).
pub(crate) fn enabled(id: &str) -> Option<bool> {
    if UNAVAILABLE.contains(&id) {
        return Some(false);
    }
    static_id(id).map(|_| true)
}

/// Entries of round-4 menu `id` (labels are English fallbacks; `docR4.*` keys translate).
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    match id {
        // Galleries answer a non-empty list so the ribbon marks them with ▾.
        id if GALLERIES.contains(&id) => vec![e(GALLERIES[0], "")],
        "insert.pageNumber" => vec![
            e("insert.pageNumber.top", "Top of Page"),
            e("insert.pageNumber.bottom", "Bottom of Page"),
            e("insert.pageNumber.margins", "Page Margins"),
            e("insert.pageNumber.current", "Current Position"),
            e("insert.pageNumber.format", "Format Page Numbers..."),
            e("insert.pageNumber.remove", "Remove Page Numbers"),
        ],
        "insert.quickParts" => vec![
            e("insert.quickParts.title", "Document Property: Title"),
            e("insert.quickParts.author", "Document Property: Author"),
            e("insert.quickParts.subject", "Document Property: Subject"),
            e("insert.quickParts.company", "Document Property: Company"),
            e("insert.quickParts.field", "Field..."),
        ],
        "insert.object" => vec![
            e("insert.object.object", "Object..."),
            e("insert.object.textFromFile", "Text from File..."),
        ],
        "insert.signatureMenu" => vec![e("insert.signatureLine", "Signature Line...")],
        "layout.position" => vec![
            e("picture.pos.inline", "In Line with Text"),
            e("picture.pos.tl", "Top Left"),
            e("picture.pos.tc", "Top Center"),
            e("picture.pos.tr", "Top Right"),
            e("picture.pos.ml", "Middle Left"),
            e("picture.pos.mc", "Middle Center"),
            e("picture.pos.mr", "Middle Right"),
            e("picture.pos.bl", "Bottom Left"),
            e("picture.pos.bc", "Bottom Center"),
            e("picture.pos.br", "Bottom Right"),
        ],
        "layout.wrapText" => vec![
            e("picture.wrap.inline", "In Line with Text"),
            e("picture.wrap.square", "Square"),
            e("picture.wrap.tight", "Tight"),
            e("picture.wrap.through", "Through"),
            e("picture.wrap.topAndBottom", "Top and Bottom"),
            e("picture.wrap.behind", "Behind Text"),
            e("picture.wrap.inFront", "In Front of Text"),
        ],
        "layout.align" => vec![
            e("picture.align.left", "Align Left"),
            e("picture.align.center", "Align Center"),
            e("picture.align.right", "Align Right"),
            e("picture.align.top", "Align Top"),
            e("picture.align.middle", "Align Middle"),
            e("picture.align.bottom", "Align Bottom"),
        ],
        "layout.rotate" => vec![
            e("picture.rotate.right", "Rotate Right 90°"),
            e("picture.rotate.left", "Rotate Left 90°"),
            e("picture.flip.vertical", "Flip Vertical"),
            e("picture.flip.horizontal", "Flip Horizontal"),
        ],
        _ => crate::doc_r4_fit::menu(id),
    }
}

/// Command labels of round 4 (id, English): the reference's Title Case.
pub(crate) const LABELS: [(&str, &str); 56] = [
    ("layout.indentLeft", "Indent Left"),
    ("layout.indentRight", "Indent Right"),
    ("layout.spaceBefore", "Spacing Before"),
    ("layout.spaceAfter", "Spacing After"),
    ("insert.coverPage", "Cover Page"),
    ("insert.blankPage", "Blank Page"),
    ("insert.pageBreak", "Page Break"),
    ("insert.table", "Table"),
    ("insert.picture", "Pictures"),
    ("insert.shapes", "Shapes"),
    ("insert.icons", "Icons"),
    ("insert.models3d", "3D Models"),
    ("insert.smartArt", "SmartArt"),
    ("insert.chart", "Chart"),
    ("insert.screenshot", "Screenshot"),
    ("insert.link", "Link"),
    ("insert.header", "Header"),
    ("insert.footer", "Footer"),
    ("insert.pageNumber", "Page Number"),
    ("insert.textBox", "Text Box"),
    ("insert.quickParts", "Quick Parts"),
    ("insert.wordArt", "WordArt"),
    ("insert.dropCap", "Drop Cap"),
    ("insert.signatureMenu", "Signature Line"),
    ("insert.dateTime", "Date & Time"),
    ("insert.object", "Object"),
    ("insert.equation", "Equation"),
    ("insert.symbol", "Symbol"),
    ("layout.margins", "Margins"),
    ("layout.orientation", "Orientation"),
    ("layout.size", "Size"),
    ("layout.columns", "Columns"),
    ("layout.breaks", "Breaks"),
    ("layout.lineNumbers", "Line Numbers"),
    ("layout.hyphenation", "Hyphenation"),
    ("layout.position", "Position"),
    ("layout.wrapText", "Wrap Text"),
    ("layout.bringForward", "Bring Forward"),
    ("layout.sendBackward", "Send Backward"),
    ("layout.selectionPane", "Selection Pane"),
    ("layout.align", "Align"),
    ("layout.group", "Group"),
    ("layout.rotate", "Rotate"),
    ("view.immersiveReader", "Immersive Reader"),
    ("view.focus", "Focus"),
    ("view.vertical", "Vertical"),
    ("view.sideToSide", "Side to Side"),
    ("view.gridlines", "Gridlines"),
    ("view.zoomDialog", "Zoom"),
    ("view.multiplePages", "Multiple Pages"),
    ("view.newWindow", "New Window"),
    ("view.arrangeAll", "Arrange All"),
    ("view.split", "Split"),
    ("view.sideBySide", "View Side by Side"),
    ("view.macros", "Macros"),
    ("view.navigationPane", "Navigation Pane"),
];

/// Group labels of round 4 (key, English).
pub(crate) const GROUPS: [(&str, &str); 11] = [
    ("docR4.groupTables", "Tables"),
    ("docR4.groupMedia", "Media"),
    ("docR4.groupParagraph", "Paragraph"),
    ("docR4.groupArrange", "Arrange"),
    ("docR4.groupImmersive", "Immersive"),
    ("docR4.groupPageMovement", "Page Movement"),
    ("docR4.groupWindow", "Window"),
    ("docR4.groupMacros", "Macros"),
    ("docR4.groupIndent", "Indent"),
    ("docR4.groupSpacing", "Spacing"),
    ("docR4.groupText", "Text"),
];

/// Title Case group labels shared with the other tabs (key, English).
const TITLE_GROUPS: [(&str, &str); 7] = [
    ("ribbon.groups.pages", "Pages"),
    ("ribbon.groups.illustrations", "Illustrations"),
    ("ribbon.groups.headerFooter", "Header & Footer"),
    ("ribbon.groups.pageSetup", "Page Setup"),
    ("ribbon.groups.views", "Views"),
    ("ribbon.groups.show", "Show"),
    ("ribbon.groups.zoom", "Zoom"),
];

/// Install the round-4 labels: `docR4.cmd.*` translations when present, else the English
/// fallbacks (en-US) of [`LABELS`].
pub(crate) fn install_strings(s: &mut crate::strings::Strings) {
    let english = s.get("ribbon.tabs.home") == "Home";
    for (id, en) in LABELS {
        let key = format!("docR4.cmd.{}", id.replace('.', "_"));
        let v = s.get(&key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        } else if english {
            s.set(&format!("commands.{id}"), en);
        }
    }
    for (key, en) in GROUPS {
        if s.get(key) == key {
            s.set(key, en);
        }
    }
    if english {
        for (key, en) in TITLE_GROUPS {
            s.set(key, en);
        }
    }
}

/// BlueOffice's own icon drawings for the round-4 buttons (24-unit grid, filled shapes in the
/// BlueDoc family: grey outlines, blue accents).
#[allow(clippy::too_many_lines)]
pub(crate) fn install_icons(icons: &mut Icons) {
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    let grey = rgb(0x6e, 0x6e, 0x6e);
    let blue = rgb(0x2f, 0x6f, 0xba);
    let pale = rgb(0xcf, 0xe2, 0xf5);
    let white = rgb(255, 255, 255);
    let green = rgb(0x3a, 0x8a, 0x4f);
    let orange = rgb(0xd8, 0x8a, 0x2a);
    let page = "M5 2h14v20h-14Z";
    let frame = "M5 2h14v20h-14ZM6 3v18h12v-18Z";
    icons.add_solid(
        "insert.coverPage",
        &[
            (page, white),
            (frame, grey),
            ("M6 3h12v9h-12Z", blue),
            ("M8 14h8v1h-8ZM8 17h5v1h-5Z", grey),
        ],
    );
    icons.add_solid(
        "insert.blankPage",
        &[
            (page, white),
            (frame, grey),
            ("M13 2l6 6h-6Z", pale),
            ("M13 2v6h6v1h-7v-7Z", grey),
        ],
    );
    icons.add_solid(
        "insert.icons",
        &[
            ("M4 4h7v7h-7Z", grey),
            ("M17 7.5m-3.5 0a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0 -7 0", blue),
            ("M7.5 13l4 7h-8Z", blue),
            ("M14 14h7v7h-7Z", grey),
        ],
    );
    icons.add_solid(
        "insert.models3d",
        &[
            ("M12 3l8 4.5v9l-8 4.5-8-4.5v-9Z", pale),
            ("M12 3l8 4.5-8 4.5-8-4.5Z", grey),
            ("M12 12v9l-1 -0.5v-8.5Z", grey),
        ],
    );
    icons.add_solid(
        "insert.smartArt",
        &[
            ("M3 5h8v5h-8Z", green),
            ("M13 5h8v5h-8Z", green),
            ("M8 14h8v5h-8Z", blue),
            ("M7 10h1v2h8v-2h1v3h-10Z", grey),
        ],
    );
    icons.add_solid(
        "insert.chart",
        &[
            ("M3 20h18v1h-18Z", grey),
            ("M5 12h3v8h-3Z", blue),
            ("M10.5 6h3v14h-3Z", orange),
            ("M16 9h3v11h-3Z", grey),
        ],
    );
    icons.add_solid(
        "insert.screenshot",
        &[
            ("M3 6h18v13h-18Z", grey),
            ("M4 7h16v11h-16Z", white),
            ("M8 10h8v5h-8Z", blue),
            ("M2 4h4v1h-3v3h-1ZM22 4v4h-1v-3h-3v-1Z", grey),
        ],
    );
    icons.add_solid(
        "insert.quickParts",
        &[
            ("M3 4h18v6h-18Z", grey),
            ("M4 5h16v4h-16Z", white),
            ("M5 6h9v2h-9Z", blue),
            ("M3 13h18v7h-18Z", grey),
            ("M4 14h16v5h-16Z", white),
            ("M5 15h12v1h-12ZM5 17h8v1h-8Z", grey),
        ],
    );
    icons.add_solid(
        "insert.wordArt",
        &[
            (
                "M5 20l6-16h2l6 16h-3l-1.5-4h-5l-1.5 4ZM10.3 13.5h3.4l-1.7-4.6Z",
                blue,
            ),
            ("M3 21h18v1h-18Z", orange),
        ],
    );
    icons.add_solid(
        "insert.object",
        &[
            (page, white),
            (frame, grey),
            ("M8 8h8v8h-8Z", pale),
            ("M8 8h8v1h-7v7h-1Z", blue),
        ],
    );
    icons.alias("insert.signatureMenu", "insert.signatureLine");
    icons.add_solid(
        "insert.dateTime",
        &[
            ("M3 5h18v16h-18Z", grey),
            ("M4 9h16v11h-16Z", white),
            (
                "M6 11h3v3h-3ZM10.5 11h3v3h-3ZM15 11h3v3h-3ZM6 15.5h3v3h-3Z",
                blue,
            ),
        ],
    );
    icons.add_solid(
        "layout.position",
        &[
            (page, white),
            (frame, grey),
            ("M7 5h10v1h-10ZM7 16h10v1h-10ZM7 19h10v1h-10Z", grey),
            ("M9 8h6v6h-6Z", blue),
        ],
    );
    icons.add_solid(
        "layout.wrapText",
        &[
            (
                "M3 4h18v1h-18ZM3 8h5v1h-5ZM3 12h5v1h-5ZM3 16h5v1h-5ZM3 20h18v1h-18Z",
                grey,
            ),
            ("M16 8h5v1h-5ZM16 12h5v1h-5ZM16 16h5v1h-5Z", grey),
            ("M9.5 7.5h5v10h-5Z", blue),
        ],
    );
    icons.add_solid(
        "layout.bringForward",
        &[
            ("M3 3h11v11h-11Z", grey),
            ("M4 4h9v9h-9Z", white),
            ("M9 9h12v12h-12Z", blue),
        ],
    );
    icons.add_solid(
        "layout.sendBackward",
        &[
            ("M3 3h11v11h-11Z", blue),
            ("M9 9h12v12h-12Z", grey),
            ("M10 10h10v10h-10Z", white),
        ],
    );
    icons.add_solid(
        "layout.selectionPane",
        &[
            ("M3 4h12v16h-12Z", grey),
            ("M4 5h10v14h-10Z", white),
            ("M6 7h6v2h-6ZM6 11h6v2h-6Z", blue),
            ("M14 11l7 7-3 0 1.5 3-1.5 0.7-1.5-3-2.5 2Z", grey),
        ],
    );
    icons.add_solid(
        "layout.align",
        &[
            ("M4 3h1v18h-1Z", grey),
            ("M6 6h12v4h-12Z", blue),
            ("M6 14h8v4h-8Z", pale),
            ("M6 14h8v1h-7v3h-1Z", blue),
        ],
    );
    icons.add_solid(
        "layout.group",
        &[
            ("M3 3h18v18h-18ZM4 4v16h16v-16Z", grey),
            ("M6 6h7v7h-7Z", blue),
            ("M11 11h7v7h-7Z", pale),
        ],
    );
    icons.add_solid(
        "layout.rotate",
        &[
            ("M5 13h9v8h-9Z", blue),
            ("M12 3a7 7 0 0 1 7 7h2l-3 4-3-4h2a5 5 0 0 0-5-5Z", grey),
        ],
    );
    icons.add_solid(
        "view.immersiveReader",
        &[
            ("M2 6c3-2 7-2 10 0v14c-3-2-7-2-10 0Z", grey),
            ("M12 6c3-2 7-2 10 0v14c-3-2-7-2-10 0Z", grey),
            (
                "M3 7.5c2.5-1.3 5.5-1.3 8 0v10.5c-2.5-1.2-5.5-1.2-8 0Z",
                white,
            ),
            (
                "M13 7.5c2.5-1.3 5.5-1.3 8 0v10.5c-2.5-1.2-5.5-1.2-8 0Z",
                white,
            ),
            ("M15 11l4-2v6l-4-2Z", blue),
        ],
    );
    icons.add_solid(
        "view.focus",
        &[
            ("M2 4h20v16h-20Z", grey),
            ("M7 6h10v12h-10Z", white),
            ("M9 9h6v1h-6ZM9 12h6v1h-6ZM9 15h4v1h-4Z", blue),
        ],
    );
    icons.add_solid(
        "view.vertical",
        &[
            (page, white),
            (frame, grey),
            ("M8 6h8v1h-8ZM8 9h8v1h-8ZM8 12h8v1h-8ZM8 15h8v1h-8Z", grey),
            ("M2 4l2-2 2 2h-1.5v16h1.5l-2 2-2-2h1.5v-16Z", blue),
        ],
    );
    icons.add_solid(
        "view.sideToSide",
        &[
            ("M2 3h9v15h-9ZM13 3h9v15h-9Z", grey),
            ("M3 4h7v13h-7ZM14 4h7v13h-7Z", white),
            ("M5 21l3-2v1.5h8v-1.5l3 2-3 2v-1.5h-8v1.5Z", blue),
        ],
    );
    icons.add_solid(
        "view.gridlines",
        &[(
            "M3 3h18v18h-18ZM4 4v4h4v-4ZM9 4v4h6v-4ZM16 4v4h4v-4ZM4 9v6h4v-6ZM9 9v6h6v-6ZM16 9v6h4v-6ZM4 16v4h4v-4ZM9 16v4h6v-4ZM16 16v4h4v-4Z",
            grey,
        )],
    );
    icons.add_solid(
        "view.zoomDialog",
        &[
            ("M10 3a7 7 0 1 1 0 14a7 7 0 1 1 0-14Z", grey),
            ("M10 5a5 5 0 1 0 0 10a5 5 0 1 0 0-10Z", white),
            ("M14.5 15.5l1.5-1.5 6 6-1.5 1.5Z", grey),
        ],
    );
    icons.add_solid(
        "view.multiplePages",
        &[
            ("M2 4h9v16h-9ZM13 4h9v16h-9Z", grey),
            ("M3 5h7v14h-7ZM14 5h7v14h-7Z", white),
            ("M4 7h5v1h-5ZM4 10h5v1h-5ZM15 7h5v1h-5ZM15 10h5v1h-5Z", blue),
        ],
    );
    icons.add_solid(
        "view.newWindow",
        &[
            ("M3 5h14v12h-14Z", grey),
            ("M4 8h12v8h-12Z", white),
            ("M7 9h14v12h-14Z", grey),
            ("M8 12h12v8h-12Z", white),
            ("M7 9h14v3h-14Z", blue),
        ],
    );
    icons.add_solid(
        "view.arrangeAll",
        &[
            ("M3 3h18v8h-18ZM3 13h18v8h-18Z", grey),
            ("M4 6h16v4h-16ZM4 16h16v4h-16Z", white),
            ("M3 3h18v3h-18ZM3 13h18v3h-18Z", blue),
        ],
    );
    icons.add_solid(
        "view.split",
        &[
            ("M3 3h18v18h-18Z", grey),
            ("M4 4h16v7h-16ZM4 13h16v7h-16Z", white),
            ("M4 11h16v2h-16Z", blue),
        ],
    );
    icons.add_solid(
        "view.sideBySide",
        &[
            ("M2 3h9v18h-9ZM13 3h9v18h-9Z", grey),
            ("M3 4h7v16h-7ZM14 4h7v16h-7Z", white),
            ("M4 6h5v1h-5ZM15 6h5v1h-5Z", blue),
        ],
    );
    icons.add_solid(
        "view.macros",
        &[
            ("M3 4h15v16h-15Z", grey),
            ("M4 5h13v14h-13Z", white),
            ("M6 8h9v1h-9ZM6 11h9v1h-9ZM6 14h5v1h-5Z", grey),
            ("M15 13h6v8h-6Z", orange),
        ],
    );
    icons.alias("layout.indentLeft", "format.indent");
    icons.alias("layout.indentRight", "format.outdent");
    icons.alias("layout.spaceBefore", "format.lineSpacing");
    icons.alias("layout.spaceAfter", "format.lineSpacing");
}

// ---------------------------------------------------------------- gallery tiles

const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const GREY: Rgba = Rgba(0x8c, 0x8c, 0x8c, 255);
const LIGHT: Rgba = Rgba(0xd9, 0xd9, 0xd9, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const ACCENT: Rgba = Rgba(0x2f, 0x6f, 0xba, 255);
const ACCENT2: Rgba = Rgba(0x9d, 0xc3, 0xe6, 255);
const WARM: Rgba = Rgba(0xd8, 0x8a, 0x2a, 255);
const EDGE: Rgba = Rgba(0xab, 0xab, 0xab, 255);

fn font(family: &'static str, size: f32, weight: u16) -> Font<'static> {
    Font {
        family,
        size,
        weight,
        italic: false,
        underline: false,
    }
}

fn frame(p: &mut dyn Painter, r: [f32; 4], fill: Rgba) {
    p.fill_rect(r[0], r[1], r[2], r[3], EDGE);
    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, fill);
}

/// A page outline in the tile (caption below): portrait pages are 0.77 × as wide as tall.
fn page_rect(r: [f32; 4], s: f32) -> [f32; 4] {
    let h = r[3] - 26.0 * s;
    let w = (h * 0.77).min(r[2] - 8.0 * s);
    [r[0] + (r[2] - w) / 2.0, r[1] + 4.0 * s, w, h]
}

#[allow(clippy::too_many_arguments)]
fn lines(p: &mut dyn Painter, x: f32, y: f32, w: f32, n: usize, step: f32, s: f32, c: Rgba) {
    for i in 0..n {
        let lw = if i + 1 == n { w * 0.6 } else { w };
        p.fill_rect(x, y + i as f32 * step, lw, s.max(1.0), c);
    }
}

/// Cover Page tiles: BlueOffice's own sixteen covers (the reference's gallery holds as many;
/// none of its designs is copied).
pub(crate) const COVERS: [(&str, &str); 16] = [
    ("horizon", "Horizon"),
    ("ledger", "Ledger"),
    ("margin", "Margin"),
    ("block", "Block"),
    ("ribbon", "Ribbon"),
    ("plain", "Plain"),
    ("slate", "Slate"),
    ("frame", "Frame"),
    ("split", "Split"),
    ("dots", "Dots"),
    ("stripes", "Stripes"),
    ("corner", "Corner"),
    ("center", "Centre Line"),
    ("column", "Column"),
    ("picture", "Picture"),
    ("duo", "Duo"),
];

/// Dark slate of the Slate cover.
const SLATE: Rgba = Rgba(0x3b, 0x47, 0x56, 255);

/// Draw cover `id` into the tile rectangle `r` (window px) at scale `s`. Every size is a
/// share of the page so the drawing fits the large tiles of the reference's gallery.
#[allow(clippy::too_many_lines)]
pub(crate) fn draw_cover(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let s = s.abs();
    // The page fills the tile above its name (the tile rectangle excludes the caption).
    let ph = r[3] - 2.0 * s;
    let pw = (ph * 0.77).min(r[2] - 8.0 * s);
    let pr = [r[0] + (r[2] - pw) / 2.0, r[1] + s, pw, ph];
    let [x, y, w, h] = pr;
    let u = w / 100.0;
    let slate = id.ends_with(".slate");
    frame(p, pr, if slate { SLATE } else { WHITE });
    let size = (h * 0.075).round().max(6.0);
    let t = font("Calibri Light", size, 400);
    let ln = s.max(1.0);
    let step = (h * 0.035).max(3.0);
    match id.rsplit('.').next() {
        Some("horizon") => {
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h * 0.45, ACCENT);
            p.text_font("TITLE", x + 7.0 * u, y + h * 0.3, &t, WHITE);
            lines(p, x + 7.0 * u, y + h * 0.55, w * 0.7, 3, step, ln, GREY);
        }
        Some("ledger") => {
            p.fill_rect(x + w * 0.62, y + 1.0, w * 0.38 - 1.0, h - 2.0, ACCENT2);
            p.text_font("Title", x + 7.0 * u, y + h * 0.35, &t, INK);
            p.fill_rect(
                x + 7.0 * u,
                y + h * 0.35 + size * 1.5,
                w * 0.45,
                2.0 * ln,
                ACCENT,
            );
            lines(p, x + w * 0.66, y + h * 0.6, w * 0.28, 4, step, ln, WHITE);
        }
        Some("margin") => {
            p.fill_rect(x + 1.0, y + 1.0, 9.0 * u, h - 2.0, ACCENT);
            p.text_font("Title", x + 16.0 * u, y + h * 0.4, &t, ACCENT);
            lines(
                p,
                x + 16.0 * u,
                y + h * 0.4 + size * 1.7,
                w * 0.6,
                2,
                step,
                ln,
                GREY,
            );
        }
        Some("block") => {
            p.fill_rect(x + 7.0 * u, y + h * 0.3, w - 14.0 * u, h * 0.28, ACCENT);
            p.text_font("TITLE", x + 12.0 * u, y + h * 0.37, &t, WHITE);
            lines(p, x + 7.0 * u, y + h * 0.75, w * 0.6, 3, step, ln, GREY);
        }
        Some("ribbon") => {
            p.fill_rect(x + 1.0, y + h * 0.18, w - 2.0, 4.0 * ln, WARM);
            p.text_font("Title", x + 7.0 * u, y + h * 0.24, &t, INK);
            p.fill_rect(x + 1.0, y + h * 0.82, w - 2.0, 4.0 * ln, WARM);
            lines(p, x + 7.0 * u, y + h * 0.88, w * 0.5, 1, step, ln, GREY);
        }
        Some("slate") => {
            p.text_font("TITLE", x + 9.0 * u, y + h * 0.42, &t, WHITE);
            p.fill_rect(
                x + 9.0 * u,
                y + h * 0.42 + size * 1.6,
                w * 0.3,
                2.0 * ln,
                WARM,
            );
            lines(p, x + 9.0 * u, y + h * 0.8, w * 0.5, 2, step, ln, LIGHT);
        }
        Some("frame") => {
            let m = 6.0 * u;
            p.fill_rect(x + m, y + m, w - 2.0 * m, h - 2.0 * m, ACCENT);
            p.fill_rect(
                x + m + 2.0 * ln,
                y + m + 2.0 * ln,
                w - 2.0 * m - 4.0 * ln,
                h - 2.0 * m - 4.0 * ln,
                WHITE,
            );
            p.text_font("Title", x + 18.0 * u, y + h * 0.4, &t, ACCENT);
            lines(
                p,
                x + 18.0 * u,
                y + h * 0.4 + size * 1.7,
                w * 0.5,
                2,
                step,
                ln,
                GREY,
            );
        }
        Some("split") => {
            p.fill_rect(x + 1.0, y + 1.0, w * 0.5, h - 2.0, ACCENT);
            p.text_font("Title", x + 6.0 * u, y + h * 0.45, &t, WHITE);
            lines(p, x + w * 0.56, y + h * 0.45, w * 0.36, 4, step, ln, GREY);
        }
        Some("dots") => {
            let d = 4.0 * u;
            for i in 0..8 {
                let cx = x + 10.0 * u + i as f32 * 10.0 * u;
                p.fill_rect(
                    cx,
                    y + h * 0.12,
                    d,
                    d,
                    if i % 2 == 0 { ACCENT } else { ACCENT2 },
                );
            }
            p.text_font("Title", x + 10.0 * u, y + h * 0.3, &t, INK);
            lines(
                p,
                x + 10.0 * u,
                y + h * 0.3 + size * 1.7,
                w * 0.6,
                3,
                step,
                ln,
                GREY,
            );
        }
        Some("stripes") => {
            for i in 0..4 {
                let c = if i % 2 == 0 { ACCENT } else { ACCENT2 };
                p.fill_rect(
                    x + 1.0,
                    y + h * (0.06 + i as f32 * 0.04),
                    w - 2.0,
                    h * 0.02,
                    c,
                );
            }
            p.text_font("Title", x + 8.0 * u, y + h * 0.4, &t, INK);
            lines(
                p,
                x + 8.0 * u,
                y + h * 0.4 + size * 1.7,
                w * 0.55,
                2,
                step,
                ln,
                GREY,
            );
        }
        Some("corner") => {
            p.fill_rect(x + w * 0.6, y + 1.0, w * 0.4 - 1.0, h * 0.22, WARM);
            p.fill_rect(x + 1.0, y + h * 0.78, w * 0.4, h * 0.22 - 1.0, ACCENT);
            p.text_font("Title", x + 8.0 * u, y + h * 0.42, &t, INK);
            lines(
                p,
                x + 8.0 * u,
                y + h * 0.42 + size * 1.7,
                w * 0.55,
                2,
                step,
                ln,
                GREY,
            );
        }
        Some("center") => {
            let tw = size * 2.6;
            p.fill_rect(x + 10.0 * u, y + h * 0.4, w - 20.0 * u, ln, GREY);
            p.text_font("Title", x + (w - tw) / 2.0, y + h * 0.42, &t, INK);
            p.fill_rect(
                x + 10.0 * u,
                y + h * 0.42 + size * 1.6,
                w - 20.0 * u,
                ln,
                GREY,
            );
            lines(p, x + 30.0 * u, y + h * 0.85, w * 0.4, 2, step, ln, GREY);
        }
        Some("column") => {
            p.fill_rect(x + w * 0.74, y + 1.0, w * 0.26 - 1.0, h - 2.0, SLATE);
            p.text_font("Title", x + 8.0 * u, y + h * 0.18, &t, INK);
            lines(
                p,
                x + 8.0 * u,
                y + h * 0.18 + size * 1.7,
                w * 0.55,
                6,
                step,
                ln,
                GREY,
            );
            lines(p, x + w * 0.78, y + h * 0.7, w * 0.18, 3, step, ln, LIGHT);
        }
        Some("picture") => {
            p.fill_rect(x + 8.0 * u, y + h * 0.08, w - 16.0 * u, h * 0.4, LIGHT);
            // A simple landscape: sun and hill (own drawing).
            p.fill_rect(x + 70.0 * u, y + h * 0.14, 8.0 * u, 8.0 * u, WARM);
            p.fill_rect(x + 8.0 * u, y + h * 0.36, w - 16.0 * u, h * 0.12, ACCENT2);
            p.text_font("Title", x + 8.0 * u, y + h * 0.55, &t, INK);
            lines(
                p,
                x + 8.0 * u,
                y + h * 0.55 + size * 1.7,
                w * 0.6,
                3,
                step,
                ln,
                GREY,
            );
        }
        Some("duo") => {
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h * 0.3, ACCENT2);
            p.fill_rect(x + 1.0, y + h * 0.3, w - 2.0, h * 0.08, ACCENT);
            p.text_font("Title", x + 8.0 * u, y + h * 0.48, &t, INK);
            lines(
                p,
                x + 8.0 * u,
                y + h * 0.48 + size * 1.7,
                w * 0.6,
                2,
                step,
                ln,
                GREY,
            );
        }
        _ => {
            p.text_font("Title", x + 7.0 * u, y + h * 0.42, &t, INK);
            lines(
                p,
                x + 7.0 * u,
                y + h * 0.42 + size * 1.7,
                w * 0.5,
                2,
                step,
                ln,
                GREY,
            );
        }
    }
}

/// Icons of the Cover Page menu rows (own drawings): a page with a globe mark (more
/// covers), a page with a cross (remove), a page with a disk (save). A negative `s` asks
/// for the dimmed look.
pub(crate) fn draw_cover_row(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let dim = s < 0.0;
    let s = s.abs();
    let [x, y, w, h] = r;
    let ink = if dim { LIGHT } else { GREY };
    let (pw, ph) = (w * 0.6, h * 0.85);
    p.fill_rect(x, y, pw, ph, ink);
    p.fill_rect(x + s, y + s, pw - 2.0 * s, ph - 2.0 * s, WHITE);
    let mark = |p: &mut dyn Painter, c: Rgba| {
        p.fill_rect(x + w * 0.45, y + h * 0.45, w * 0.55, h * 0.55, c);
    };
    match id.rsplit('.').next() {
        Some("remove") => {
            let c = if dim {
                LIGHT
            } else {
                Rgba(0xc4, 0x2b, 0x1c, 255)
            };
            let (cx, cy, k) = (x + w * 0.72, y + h * 0.72, w * 0.22);
            p.path(
                &[
                    PathSeg::M(cx - k, cy - k),
                    PathSeg::L(cx + k, cy + k),
                    PathSeg::M(cx + k, cy - k),
                    PathSeg::L(cx - k, cy + k),
                ],
                None,
                Some((c, 1.5 * s)),
            );
        }
        Some("more") => mark(p, if dim { LIGHT } else { ACCENT }),
        _ => mark(p, if dim { LIGHT } else { ACCENT2 }),
    }
}

/// Header / footer tiles (wide, caption on top): Blank, Blank (Three Columns) and
/// BlueOffice's own designs.
pub(crate) const BANDS: [(&str, &str); 5] = [
    ("blank", "Blank"),
    ("three", "Blank (Three Columns)"),
    ("rule", "Rule"),
    ("band", "Band"),
    ("title", "Title and Page"),
];

pub(crate) fn draw_band(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let header = id.starts_with("insert.header");
    let [x, y, w, h] = r;
    let top = y + 20.0 * s;
    let body = [x + 2.0 * s, top, w - 4.0 * s, h - 22.0 * s];
    frame(p, body, WHITE);
    let f = font("Calibri", (9.0 * s).round(), 400);
    let mid = body[1] + body[3] / 2.0 - 5.0 * s;
    let line_y = if header {
        body[1] + body[3] - 10.0 * s
    } else {
        body[1] + 8.0 * s
    };
    match id.rsplit('.').next() {
        Some("three") => {
            for k in 0..3 {
                let tx = body[0] + 12.0 * s + k as f32 * (body[2] - 24.0 * s) / 2.6;
                p.text_font("[Type here]", tx, mid, &f, INK);
            }
        }
        Some("rule") => {
            p.text_font("[Document title]", body[0] + 12.0 * s, mid, &f, ACCENT);
            p.fill_rect(
                body[0] + 10.0 * s,
                line_y,
                body[2] - 20.0 * s,
                s.max(1.0),
                ACCENT,
            );
        }
        Some("band") => {
            p.fill_rect(
                body[0] + 10.0 * s,
                mid - 3.0 * s,
                body[2] - 20.0 * s,
                16.0 * s,
                ACCENT,
            );
            p.text_font("[DOCUMENT TITLE]", body[0] + 16.0 * s, mid, &f, WHITE);
        }
        Some("title") => {
            p.text_font("[Document title]", body[0] + 12.0 * s, mid, &f, INK);
            p.text_font("1", body[0] + body[2] - 20.0 * s, mid, &f, ACCENT);
        }
        _ => p.text_font("[Type here]", body[0] + 12.0 * s, mid, &f, INK),
    }
}

/// HTML of a header / footer design (pasted into the story).
pub(crate) fn band_html(design: &str) -> &'static str {
    match design {
        "three" => {
            "<table style=\"width:100%;border:none\"><tr><td>[Type here]</td>\
             <td style=\"text-align:center\">[Type here]</td>\
             <td style=\"text-align:right\">[Type here]</td></tr></table>"
        }
        "rule" => "<p style=\"color:#2F6FBA;border-bottom:1px solid #2F6FBA\">[Document title]</p>",
        "band" => {
            "<p style=\"background:#2F6FBA;color:#FFFFFF;text-align:center\">[DOCUMENT TITLE]</p>"
        }
        "title" => "<p>[Document title]</p>",
        _ => "<p>[Type here]</p>",
    }
}

/// HTML of a cover page (pasted at the document's start, then a page break).
pub(crate) fn cover_html(design: &str) -> &'static str {
    match design {
        "horizon" => {
            "<p style=\"background:#2F6FBA;color:#FFFFFF;font-size:36pt\">[DOCUMENT TITLE]</p>\
             <p style=\"color:#2F6FBA;font-size:16pt\">[Document subtitle]</p>\
             <p>[Author name]</p><p>[Date]</p>"
        }
        "ledger" => {
            "<p style=\"font-size:36pt\">[Document title]</p>\
             <p style=\"border-bottom:2px solid #2F6FBA;font-size:14pt\">[Document subtitle]</p>\
             <p>[Abstract]</p>"
        }
        "margin" => {
            "<p style=\"color:#2F6FBA;font-size:36pt;border-left:6px solid #2F6FBA\">[Document title]</p>\
             <p>[Document subtitle]</p><p>[Author name]</p>"
        }
        "block" => {
            "<p style=\"background:#2F6FBA;color:#FFFFFF;text-align:center;font-size:32pt\">[DOCUMENT TITLE]</p>\
             <p style=\"text-align:center\">[Document subtitle]</p>"
        }
        "ribbon" => {
            "<p style=\"border-top:4px solid #D88A2A;font-size:36pt\">[Document title]</p>\
             <p style=\"border-bottom:4px solid #D88A2A\">[Company name] | [Date]</p>"
        }
        "slate" => {
            "<p style=\"background:#3B4756;color:#FFFFFF;font-size:36pt\">[DOCUMENT TITLE]</p>\
             <p style=\"border-bottom:2px solid #D88A2A\">[Document subtitle]</p><p>[Author name]</p>"
        }
        "frame" => {
            "<p style=\"border:3px solid #2F6FBA;color:#2F6FBA;font-size:36pt;text-align:center\">[Document title]</p>\
             <p style=\"text-align:center\">[Document subtitle]</p>"
        }
        "split" => {
            "<p style=\"background:#2F6FBA;color:#FFFFFF;font-size:36pt\">[Document title]</p>\
             <p>[Document subtitle]</p><p>[Abstract]</p>"
        }
        "dots" => {
            "<p style=\"color:#2F6FBA\">&#9679; &#9679; &#9679; &#9679;</p>\
             <p style=\"font-size:36pt\">[Document title]</p><p>[Document subtitle]</p>"
        }
        "stripes" => {
            "<p style=\"border-top:6px double #2F6FBA;font-size:36pt\">[Document title]</p>\
             <p>[Document subtitle]</p><p>[Author name]</p>"
        }
        "corner" => {
            "<p style=\"text-align:right;color:#D88A2A\">[Date]</p>\
             <p style=\"font-size:36pt\">[Document title]</p><p style=\"color:#2F6FBA\">[Company name]</p>"
        }
        "center" => {
            "<p style=\"border-top:1px solid #8C8C8C;border-bottom:1px solid #8C8C8C;font-size:36pt;text-align:center\">[Document title]</p>\
             <p style=\"text-align:center\">[Document subtitle]</p>"
        }
        "column" => {
            "<p style=\"font-size:36pt\">[Document title]</p>\
             <p style=\"border-left:6px solid #3B4756\">[Abstract]</p><p>[Author name]</p>"
        }
        "picture" => {
            "<p style=\"background:#D9D9D9;text-align:center\">[Picture]</p>\
             <p style=\"font-size:36pt\">[Document title]</p><p>[Document subtitle]</p>"
        }
        "duo" => {
            "<p style=\"background:#9DC3E6;border-bottom:6px solid #2F6FBA;font-size:36pt\">[Document title]</p>\
             <p>[Document subtitle]</p>"
        }
        _ => "<p style=\"font-size:36pt\">[Document title]</p><p>[Document subtitle]</p>",
    }
}

/// Text Box tiles: a page with the box placed; BlueOffice's own designs.
pub(crate) const BOXES: [(&str, &str); 6] = [
    ("simple", "Simple Text Box"),
    ("quote", "Pull Quote"),
    ("sidebar", "Sidebar"),
    ("band", "Banner"),
    ("note", "Note"),
    ("corner", "Corner Quote"),
];

pub(crate) fn draw_box(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let pr = page_rect(r, s);
    frame(p, pr, WHITE);
    let [x, y, w, h] = pr;
    let (bx, by, bw, bh, fill) = match id.rsplit('.').next() {
        Some("quote") => (x + w * 0.15, y + h * 0.1, w * 0.7, h * 0.12, WHITE),
        Some("sidebar") => (x + w * 0.68, y + 2.0, w * 0.3, h - 4.0, ACCENT2),
        Some("band") => (x + 2.0, y + h * 0.42, w - 4.0, h * 0.14, ACCENT),
        Some("note") => (x + w * 0.1, y + h * 0.6, w * 0.8, h * 0.25, LIGHT),
        Some("corner") => (x + w * 0.5, y + h * 0.65, w * 0.45, h * 0.3, WHITE),
        _ => (x + w * 0.1, y + h * 0.08, w * 0.45, h * 0.2, WHITE),
    };
    p.fill_rect(bx, by, bw, bh, INK);
    p.fill_rect(bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0, fill);
    let c = if fill == ACCENT { WHITE } else { GREY };
    lines(
        p,
        bx + 3.0 * s,
        by + 3.0 * s,
        (bw - 6.0 * s).max(1.0),
        ((bh - 4.0 * s) / (4.0 * s)).max(1.0) as usize,
        4.0 * s,
        s,
        c,
    );
}

/// Equation tiles (caption on top): well-known formulas.
pub(crate) const EQUATIONS: [(&str, &str, &str, &str); 5] = [
    ("circle", "Area of Circle", "A = π r²", "A=\\pi r^2"),
    (
        "pythagoras",
        "Pythagorean Theorem",
        "a² + b² = c²",
        "a^2+b^2=c^2",
    ),
    (
        "quadratic",
        "Quadratic Formula",
        "x = (−b ± √(b² − 4ac)) / 2a",
        "x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}",
    ),
    (
        "binomial",
        "Binomial Theorem",
        "(x + a)ⁿ = Σ C(n,k) xᵏ aⁿ⁻ᵏ",
        "(x+a)^n=\\sum_{k=0}^{n}\\binom{n}{k}x^k a^{n-k}",
    ),
    (
        "trig",
        "Trig Identity 1",
        "sin²α + cos²α = 1",
        "\\sin^2\\alpha+\\cos^2\\alpha=1",
    ),
];

pub(crate) fn draw_equation(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let [x, y, w, h] = r;
    let body = [x + 2.0 * s, y + 20.0 * s, w - 4.0 * s, h - 22.0 * s];
    frame(p, body, WHITE);
    let key = id.rsplit('.').next().unwrap_or("");
    let shown = EQUATIONS.iter().find(|e| e.0 == key).map_or("", |e| e.2);
    let f = Font {
        family: "Cambria Math",
        size: (14.0 * s).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    p.text_font(
        shown,
        body[0] + 8.0 * s,
        body[1] + body[3] / 2.0 - 9.0 * s,
        &f,
        INK,
    );
}

/// The linear (LaTeX-like) form of equation tile `key`.
pub(crate) fn equation_text(key: &str) -> Option<&'static str> {
    EQUATIONS.iter().find(|e| e.0 == key).map(|e| e.3)
}

/// Shapes gallery sections: (header, presets).
pub(crate) const SHAPES: [(&str, &[&str]); 5] = [
    ("Lines", &["line", "straightConnector1", "bentConnector3"]),
    (
        "Rectangles",
        &["rect", "roundRect", "snip1Rect", "round2SameRect"],
    ),
    (
        "Basic Shapes",
        &[
            "ellipse",
            "triangle",
            "rtTriangle",
            "parallelogram",
            "trapezoid",
            "diamond",
            "pentagon",
            "hexagon",
            "octagon",
            "plus",
            "donut",
            "can",
        ],
    ),
    (
        "Block Arrows",
        &[
            "rightArrow",
            "leftArrow",
            "upArrow",
            "downArrow",
            "leftRightArrow",
            "chevron",
        ],
    ),
    ("Stars and Banners", &["star4", "star5", "star6", "wave"]),
];

fn poly(p: &mut dyn Painter, pts: &[(f32, f32)], fill: Option<Rgba>, stroke: Rgba, s: f32) {
    let mut segs = Vec::with_capacity(pts.len() + 1);
    for (i, &(x, y)) in pts.iter().enumerate() {
        segs.push(if i == 0 {
            PathSeg::M(x, y)
        } else {
            PathSeg::L(x, y)
        });
    }
    segs.push(PathSeg::Z);
    p.path(&segs, fill, Some((stroke, s.max(1.0))));
}

/// A shape preset drawn as an outline in a small tile (own drawings of the geometry).
pub(crate) fn draw_shape(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let preset = id.rsplit('.').next().unwrap_or("");
    let m = 4.0 * s;
    let (x0, y0, x1, y1) = (r[0] + m, r[1] + m, r[0] + r[2] - m, r[1] + r[3] - m);
    let (cx, cy) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
    let (w, h) = (x1 - x0, y1 - y0);
    let c = Rgba(0x40, 0x40, 0x40, 255);
    let pts: Vec<(f32, f32)> = match preset {
        "line" | "straightConnector1" => {
            p.path(
                &[PathSeg::M(x0, y1), PathSeg::L(x1, y0)],
                None,
                Some((c, s.max(1.0))),
            );
            if preset == "straightConnector1" {
                poly(
                    p,
                    &[
                        (x1, y0),
                        (x1 - 5.0 * s, y0 + 1.0 * s),
                        (x1 - 1.0 * s, y0 + 5.0 * s),
                    ],
                    Some(c),
                    c,
                    s,
                );
            }
            return;
        }
        "bentConnector3" => {
            p.path(
                &[
                    PathSeg::M(x0, y0),
                    PathSeg::L(cx, y0),
                    PathSeg::L(cx, y1),
                    PathSeg::L(x1, y1),
                ],
                None,
                Some((c, s.max(1.0))),
            );
            return;
        }
        "ellipse" | "donut" => {
            let n = 28;
            let ring = |rx: f32, ry: f32| -> Vec<(f32, f32)> {
                (0..n)
                    .map(|i| {
                        let a = i as f32 / n as f32 * std::f32::consts::TAU;
                        (cx + rx * a.cos(), cy + ry * a.sin())
                    })
                    .collect()
            };
            poly(p, &ring(w / 2.0, h / 2.0), None, c, s);
            if preset == "donut" {
                poly(p, &ring(w / 4.0, h / 4.0), None, c, s);
            }
            return;
        }
        "can" => {
            poly(
                p,
                &[(x0, y0 + 3.0 * s), (x1, y0 + 3.0 * s), (x1, y1), (x0, y1)],
                None,
                c,
                s,
            );
            poly(
                p,
                &[
                    (x0, y0 + 3.0 * s),
                    (cx, y0),
                    (x1, y0 + 3.0 * s),
                    (cx, y0 + 6.0 * s),
                ],
                None,
                c,
                s,
            );
            return;
        }

        "roundRect" | "round2SameRect" => {
            let k = 4.0 * s;
            if preset == "roundRect" {
                vec![
                    (x0 + k, y0),
                    (x1 - k, y0),
                    (x1, y0 + k),
                    (x1, y1 - k),
                    (x1 - k, y1),
                    (x0 + k, y1),
                    (x0, y1 - k),
                    (x0, y0 + k),
                ]
            } else {
                vec![
                    (x0 + k, y0),
                    (x1 - k, y0),
                    (x1, y0 + k),
                    (x1, y1),
                    (x0, y1),
                    (x0, y0 + k),
                ]
            }
        }
        "snip1Rect" => vec![
            (x0, y0),
            (x1 - 5.0 * s, y0),
            (x1, y0 + 5.0 * s),
            (x1, y1),
            (x0, y1),
        ],
        "triangle" => vec![(cx, y0), (x1, y1), (x0, y1)],
        "rtTriangle" => vec![(x0, y0), (x1, y1), (x0, y1)],
        "parallelogram" => vec![(x0 + w * 0.25, y0), (x1, y0), (x1 - w * 0.25, y1), (x0, y1)],
        "trapezoid" => vec![(x0 + w * 0.25, y0), (x1 - w * 0.25, y0), (x1, y1), (x0, y1)],
        "diamond" => vec![(cx, y0), (x1, cy), (cx, y1), (x0, cy)],
        "pentagon" | "hexagon" | "octagon" => {
            let n = match preset {
                "pentagon" => 5,
                "hexagon" => 6,
                _ => 8,
            };
            (0..n)
                .map(|i| {
                    let a =
                        -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
                    (cx + w / 2.0 * a.cos(), cy + h / 2.0 * a.sin())
                })
                .collect()
        }
        "plus" => {
            let (a, b) = (w / 3.0, h / 3.0);
            vec![
                (x0 + a, y0),
                (x1 - a, y0),
                (x1 - a, y0 + b),
                (x1, y0 + b),
                (x1, y1 - b),
                (x1 - a, y1 - b),
                (x1 - a, y1),
                (x0 + a, y1),
                (x0 + a, y1 - b),
                (x0, y1 - b),
                (x0, y0 + b),
                (x0 + a, y0 + b),
            ]
        }
        "rightArrow" | "leftArrow" | "upArrow" | "downArrow" => {
            let arrow = [
                (0.0, 0.3),
                (0.55, 0.3),
                (0.55, 0.0),
                (1.0, 0.5),
                (0.55, 1.0),
                (0.55, 0.7),
                (0.0, 0.7),
            ];
            arrow
                .iter()
                .map(|&(u, v)| {
                    let (u, v) = match preset {
                        "leftArrow" => (1.0 - u, v),
                        "upArrow" => (v, 1.0 - u),
                        "downArrow" => (v, u),
                        _ => (u, v),
                    };
                    (x0 + u * w, y0 + v * h)
                })
                .collect()
        }
        "leftRightArrow" => [
            (0.0, 0.5),
            (0.3, 0.0),
            (0.3, 0.3),
            (0.7, 0.3),
            (0.7, 0.0),
            (1.0, 0.5),
            (0.7, 1.0),
            (0.7, 0.7),
            (0.3, 0.7),
            (0.3, 1.0),
        ]
        .iter()
        .map(|&(u, v)| (x0 + u * w, y0 + v * h))
        .collect(),
        "chevron" => [
            (0.0, 0.0),
            (0.6, 0.0),
            (1.0, 0.5),
            (0.6, 1.0),
            (0.0, 1.0),
            (0.4, 0.5),
        ]
        .iter()
        .map(|&(u, v)| (x0 + u * w, y0 + v * h))
        .collect(),
        "star4" | "star5" | "star6" => {
            let n = match preset {
                "star4" => 4,
                "star5" => 5,
                _ => 6,
            };
            let inner = if n == 4 { 0.25 } else { 0.42 };
            (0..2 * n)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2
                        + i as f32 / (2 * n) as f32 * std::f32::consts::TAU;
                    let k = if i % 2 == 0 { 0.5 } else { 0.5 * inner };
                    (cx + w * k * a.cos(), cy + h * k * a.sin())
                })
                .collect()
        }
        "wave" => {
            let n = 12;
            let mut v: Vec<(f32, f32)> = (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    (
                        x0 + t * w,
                        y0 + h * 0.2 + (t * std::f32::consts::TAU).sin() * h * 0.12,
                    )
                })
                .collect();
            v.extend((0..=n).rev().map(|i| {
                let t = i as f32 / n as f32;
                (
                    x0 + t * w,
                    y1 - h * 0.2 + (t * std::f32::consts::TAU).sin() * h * 0.12,
                )
            }));
            v
        }
        _ => vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
    };
    poly(p, &pts, None, c, s);
}

/// SmartArt tiles: layouts of the shared diagram engine (id, caption).
pub(crate) const SMARTART: [(&str, &str); 8] = [
    ("list", "Basic List"),
    ("process", "Basic Process"),
    ("processChevron", "Chevron Process"),
    ("cycle", "Basic Cycle"),
    ("hierarchy", "Hierarchy"),
    ("matrix", "Basic Matrix"),
    ("pyramid", "Basic Pyramid"),
    ("relationship", "Basic Venn"),
];

pub(crate) fn draw_smartart(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let [x, y, w, _] = r;
    let a = [x + 8.0 * s, y + 6.0 * s, w - 16.0 * s, 46.0 * s];
    let key = id.rsplit('.').next().unwrap_or("");
    let boxf = |p: &mut dyn Painter, bx: f32, by: f32, bw: f32, bh: f32| {
        p.fill_rect(bx, by, bw, bh, ACCENT);
    };
    match key {
        "list" => {
            for i in 0..3 {
                boxf(
                    p,
                    a[0] + i as f32 * a[2] / 3.0,
                    a[1] + 10.0 * s,
                    a[2] / 3.0 - 3.0 * s,
                    26.0 * s,
                );
            }
        }
        "process" | "processChevron" => {
            for i in 0..3 {
                let bx = a[0] + i as f32 * a[2] / 3.0;
                if key == "processChevron" {
                    poly(
                        p,
                        &[
                            (bx, a[1] + 10.0 * s),
                            (bx + a[2] / 3.0 - 8.0 * s, a[1] + 10.0 * s),
                            (bx + a[2] / 3.0 - 1.0 * s, a[1] + 23.0 * s),
                            (bx + a[2] / 3.0 - 8.0 * s, a[1] + 36.0 * s),
                            (bx, a[1] + 36.0 * s),
                            (bx + 7.0 * s, a[1] + 23.0 * s),
                        ],
                        Some(ACCENT),
                        ACCENT,
                        s,
                    );
                } else {
                    boxf(p, bx, a[1] + 12.0 * s, a[2] / 3.0 - 10.0 * s, 22.0 * s);
                    if i < 2 {
                        let ax = bx + a[2] / 3.0 - 9.0 * s;
                        poly(
                            p,
                            &[
                                (ax, a[1] + 19.0 * s),
                                (ax + 7.0 * s, a[1] + 23.0 * s),
                                (ax, a[1] + 27.0 * s),
                            ],
                            Some(ACCENT2),
                            ACCENT2,
                            s,
                        );
                    }
                }
            }
        }
        "cycle" => {
            let (cx, cy) = (a[0] + a[2] / 2.0, a[1] + 23.0 * s);
            for i in 0..5 {
                let t = -std::f32::consts::FRAC_PI_2 + i as f32 / 5.0 * std::f32::consts::TAU;
                let (bx, by) = (cx + 17.0 * s * t.cos(), cy + 17.0 * s * t.sin());
                boxf(p, bx - 5.0 * s, by - 5.0 * s, 10.0 * s, 10.0 * s);
            }
        }
        "hierarchy" => {
            let cx = a[0] + a[2] / 2.0;
            boxf(p, cx - 12.0 * s, a[1], 24.0 * s, 13.0 * s);
            p.fill_rect(cx - 0.5, a[1] + 13.0 * s, s.max(1.0), 6.0 * s, GREY);
            p.fill_rect(cx - 26.0 * s, a[1] + 19.0 * s, 52.0 * s, s.max(1.0), GREY);
            for k in [-1.0, 1.0] {
                boxf(
                    p,
                    cx + k * 26.0 * s - 12.0 * s,
                    a[1] + 24.0 * s,
                    24.0 * s,
                    13.0 * s,
                );
            }
        }
        "matrix" => {
            let cx = a[0] + a[2] / 2.0;
            for (i, j) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                boxf(
                    p,
                    cx - 22.0 * s + i * 23.0 * s,
                    a[1] + j * 23.0 * s,
                    21.0 * s,
                    21.0 * s,
                );
            }
        }
        "pyramid" => {
            let cx = a[0] + a[2] / 2.0;
            poly(
                p,
                &[
                    (cx, a[1]),
                    (cx + 26.0 * s, a[1] + 44.0 * s),
                    (cx - 26.0 * s, a[1] + 44.0 * s),
                ],
                Some(ACCENT),
                WHITE,
                s,
            );
            p.fill_rect(cx - 30.0 * s, a[1] + 15.0 * s, 60.0 * s, 2.0 * s, WHITE);
            p.fill_rect(cx - 30.0 * s, a[1] + 30.0 * s, 60.0 * s, 2.0 * s, WHITE);
        }
        _ => {
            let cy = a[1] + 23.0 * s;
            for k in [-1.0, 1.0] {
                let cx = a[0] + a[2] / 2.0 + k * 9.0 * s;
                let pts: Vec<(f32, f32)> = (0..24)
                    .map(|i| {
                        let t = i as f32 / 24.0 * std::f32::consts::TAU;
                        (cx + 16.0 * s * t.cos(), cy + 16.0 * s * t.sin())
                    })
                    .collect();
                poly(p, &pts, None, ACCENT, 2.0 * s);
            }
        }
    }
}

/// Chart tiles: chart types of the shared chart engine (kind, caption).
pub(crate) const CHARTS: [(&str, &str); 6] = [
    ("column", "Column"),
    ("line", "Line"),
    ("pie", "Pie"),
    ("bar", "Bar"),
    ("area", "Area"),
    ("scatter", "X Y (Scatter)"),
];

pub(crate) fn draw_chart(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let [x, y, w, _] = r;
    let (x0, y0, cw, ch) = (x + 10.0 * s, y + 6.0 * s, w - 20.0 * s, 40.0 * s);
    p.fill_rect(x0, y0 + ch, cw, s.max(1.0), GREY);
    let vals = [0.5, 0.8, 0.35, 0.65];
    match id.rsplit('.').next() {
        Some("column") => {
            for (i, v) in vals.iter().enumerate() {
                let bw = cw / 6.0;
                p.fill_rect(
                    x0 + 4.0 * s + i as f32 * bw * 1.4,
                    y0 + ch * (1.0 - v),
                    bw,
                    ch * v,
                    if i % 2 == 0 { ACCENT } else { WARM },
                );
            }
        }
        Some("bar") => {
            for (i, v) in vals.iter().enumerate() {
                p.fill_rect(
                    x0,
                    y0 + i as f32 * 10.0 * s,
                    cw * v,
                    7.0 * s,
                    if i % 2 == 0 { ACCENT } else { WARM },
                );
            }
        }
        Some("pie") => {
            let (cx, cy, rr) = (x0 + cw / 2.0, y0 + ch / 2.0, ch / 2.0);
            let wedge = |p: &mut dyn Painter, a0: f32, a1: f32, c: Rgba| {
                let mut pts = vec![(cx, cy)];
                let n = 16;
                for i in 0..=n {
                    let t = a0 + (a1 - a0) * i as f32 / n as f32;
                    pts.push((cx + rr * t.cos(), cy + rr * t.sin()));
                }
                poly(p, &pts, Some(c), WHITE, s);
            };
            wedge(p, -1.57, 1.2, ACCENT);
            wedge(p, 1.2, 3.3, WARM);
            wedge(p, 3.3, 4.71, ACCENT2);
        }
        Some("area") => {
            let mut pts = vec![(x0, y0 + ch)];
            for (i, v) in vals.iter().enumerate() {
                pts.push((x0 + i as f32 * cw / 3.0, y0 + ch * (1.0 - v)));
            }
            pts.push((x0 + cw, y0 + ch));
            poly(p, &pts, Some(ACCENT), ACCENT, s);
        }
        Some("scatter") => {
            for (i, v) in [0.3, 0.7, 0.5, 0.9, 0.2, 0.6].iter().enumerate() {
                let px = x0 + 4.0 * s + i as f32 * cw / 6.5;
                p.fill_rect(px, y0 + ch * (1.0 - v), 4.0 * s, 4.0 * s, ACCENT);
            }
        }
        _ => {
            let segs: Vec<PathSeg> = vals
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let pt = (x0 + i as f32 * cw / 3.0, y0 + ch * (1.0 - v));
                    if i == 0 {
                        PathSeg::M(pt.0, pt.1)
                    } else {
                        PathSeg::L(pt.0, pt.1)
                    }
                })
                .collect();
            p.path(&segs, None, Some((ACCENT, 2.0 * s)));
        }
    }
}

/// WordArt styles: BlueOffice's own fills and outlines (id, fill, outline).
pub(crate) const WORDART: [(&str, &str, Option<&str>); 10] = [
    ("plain", "262626", None),
    ("blue", "2F6FBA", None),
    ("outline", "FFFFFF", Some("2F6FBA")),
    ("warm", "D88A2A", None),
    ("warmOutline", "FFFFFF", Some("D88A2A")),
    ("green", "3A8A4F", None),
    ("grey", "7F7F7F", None),
    ("dark", "1F3864", Some("9DC3E6")),
    ("red", "C0392B", None),
    ("light", "9DC3E6", Some("1F3864")),
];

pub(crate) fn draw_wordart(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let key = id.rsplit('.').next().unwrap_or("");
    let Some((_, fill, outline)) = WORDART.iter().find(|w| w.0 == key) else {
        return;
    };
    let c = crate::theme::parse_color(&format!("#{fill}")).unwrap_or(INK);
    let f = font("Arial Black", (22.0 * s).round(), 900);
    if let Some(o) = outline.and_then(|o| crate::theme::parse_color(&format!("#{o}"))) {
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            p.text_font(
                "A",
                r[0] + 14.0 * s + dx * s,
                r[1] + 6.0 * s + dy * s,
                &f,
                o,
            );
        }
    }
    p.text_font("A", r[0] + 14.0 * s, r[1] + 6.0 * s, &f, c);
}

/// Margins presets (id, caption, top, bottom, left, right in cm).
pub(crate) const MARGINS: [(&str, &str, [f32; 4]); 6] = [
    ("normal", "Normal", [2.54, 2.54, 2.54, 2.54]),
    ("narrow", "Narrow", [1.27, 1.27, 1.27, 1.27]),
    ("moderate", "Moderate", [2.54, 2.54, 1.91, 1.91]),
    ("wide", "Wide", [2.54, 2.54, 5.08, 5.08]),
    ("mirrored", "Mirrored", [2.54, 2.54, 3.18, 2.54]),
    ("vn", "Vietnam (Decree 30)", [2.0, 2.0, 3.0, 1.5]),
];

/// A margins row: page icon with the margin lines, then the caption and values.
pub(crate) fn draw_margins(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let key = id.rsplit('.').next().unwrap_or("");
    let Some((_, name, m)) = MARGINS.iter().find(|x| x.0 == key) else {
        return;
    };
    let pw = 40.0 * s;
    let ph = r[3] - 12.0 * s;
    let (px, py) = (r[0] + 8.0 * s, r[1] + 6.0 * s);
    frame(p, [px, py, pw, ph], WHITE);
    let k = pw / 21.0;
    let (t, b, l, rr) = (m[0] * k, m[1] * k, m[2] * k, m[3] * k);
    let c = Rgba(0x9d, 0x9d, 0x9d, 255);
    p.fill_rect(px + l, py + 1.0, 1.0, ph - 2.0, c);
    p.fill_rect(px + pw - rr, py + 1.0, 1.0, ph - 2.0, c);
    p.fill_rect(px + 1.0, py + t, pw - 2.0, 1.0, c);
    p.fill_rect(px + 1.0, py + ph - b, pw - 2.0, 1.0, c);
    let bold = font("Segoe UI", (12.0 * s).round(), 700);
    let small = font("Segoe UI", (12.0 * s).round(), 400);
    let tx = px + pw + 12.0 * s;
    p.text_font(name, tx, r[1] + 4.0 * s, &bold, INK);
    let fmt = |v: f32| format!("{v:.2} cm");
    p.text_font(
        &format!("Top:  {}    Bottom: {}", fmt(m[0]), fmt(m[1])),
        tx,
        r[1] + 22.0 * s,
        &small,
        INK,
    );
    p.text_font(
        &format!("Left:  {}    Right:   {}", fmt(m[2]), fmt(m[3])),
        tx,
        r[1] + 38.0 * s,
        &small,
        INK,
    );
}

/// Orientation rows (id, caption).
pub(crate) const ORIENTATIONS: [(&str, &str); 2] =
    [("portrait", "Portrait"), ("landscape", "Landscape")];

pub(crate) fn draw_orientation(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let landscape = id.ends_with("landscape");
    let (pw, ph) = if landscape {
        (34.0 * s, 26.0 * s)
    } else {
        (26.0 * s, 34.0 * s)
    };
    let (px, py) = (
        r[0] + 8.0 * s + (34.0 * s - pw) / 2.0,
        r[1] + (r[3] - ph) / 2.0,
    );
    frame(p, [px, py, pw, ph], WHITE);
    lines(
        p,
        px + 4.0 * s,
        py + 5.0 * s,
        pw - 8.0 * s,
        4,
        5.0 * s,
        s,
        GREY,
    );
    let f = font("Segoe UI", (12.0 * s).round(), 400);
    let name = if landscape { "Landscape" } else { "Portrait" };
    p.text_font(name, r[0] + 54.0 * s, r[1] + r[3] / 2.0 - 9.0 * s, &f, INK);
}

/// Paper sizes (id, caption, width × height cm).
pub(crate) const SIZES: [(&str, &str, f32, f32); 6] = [
    ("letter", "Letter", 21.59, 27.94),
    ("legal", "Legal", 21.59, 35.56),
    ("a3", "A3", 29.7, 42.0),
    ("a4", "A4", 21.0, 29.7),
    ("a5", "A5", 14.8, 21.0),
    ("b5", "B5 (JIS)", 18.2, 25.7),
];

pub(crate) fn draw_size(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let key = id.rsplit('.').next().unwrap_or("");
    let Some((_, name, w, h)) = SIZES.iter().find(|x| x.0 == key) else {
        return;
    };
    let ph = r[3] - 12.0 * s;
    let pw = ph * 0.75;
    frame(p, [r[0] + 10.0 * s, r[1] + 6.0 * s, pw, ph], WHITE);
    let bold = font("Segoe UI", (12.0 * s).round(), 700);
    let small = font("Segoe UI", (12.0 * s).round(), 400);
    let tx = r[0] + 22.0 * s + pw;
    p.text_font(name, tx, r[1] + 6.0 * s, &bold, INK);
    p.text_font(
        &format!("{w} cm x {h} cm"),
        tx,
        r[1] + 24.0 * s,
        &small,
        INK,
    );
}

/// Column presets (id, caption).
pub(crate) const COLUMNS: [(&str, &str); 5] = [
    ("1", "One"),
    ("2", "Two"),
    ("3", "Three"),
    ("left", "Left"),
    ("right", "Right"),
];

pub(crate) fn draw_columns(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let key = id.rsplit('.').next().unwrap_or("");
    let (pw, ph) = (30.0 * s, r[3] - 10.0 * s);
    let (px, py) = (r[0] + 8.0 * s, r[1] + 5.0 * s);
    frame(p, [px, py, pw, ph], WHITE);
    let inner = pw - 8.0 * s;
    let widths: Vec<f32> = match key {
        "2" => vec![0.5, 0.5],
        "3" => vec![1.0 / 3.0; 3],
        "left" => vec![0.33, 0.67],
        "right" => vec![0.67, 0.33],
        _ => vec![1.0],
    };
    let mut x = px + 4.0 * s;
    for f in &widths {
        let cw = inner * f - 2.0 * s;
        lines(p, x, py + 4.0 * s, cw.max(1.0), 6, 4.0 * s, s, GREY);
        x += inner * f;
    }
    let f = font("Segoe UI", (12.0 * s).round(), 400);
    let name = COLUMNS.iter().find(|c| c.0 == key).map_or("", |c| c.1);
    p.text_font(name, r[0] + 50.0 * s, r[1] + r[3] / 2.0 - 9.0 * s, &f, INK);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_round4_id_is_static_and_menus_have_entries() {
        for id in GALLERIES.iter().chain(MENUS.iter()) {
            assert!(!menu(id).is_empty(), "{id}");
            assert!(static_id(id).is_some(), "{id}");
        }
        for id in UNAVAILABLE {
            assert_eq!(enabled(id), Some(false), "{id}");
        }
        assert_eq!(enabled("insert.coverPage"), Some(true));
        assert_eq!(enabled("edit.copy"), None);
        assert_eq!(equation_text("circle"), Some("A=\\pi r^2"));
        assert!(cover_html("horizon").contains("TITLE"));
    }
}
