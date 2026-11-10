//! Built-in document templates, authored in code by BlueOffice.
//!
//! The administrative templates follow the presentation rules of Nghị định
//! 30/2020/NĐ-CP (Phụ lục I), encoded once in [`nd30`] and shared with the format
//! checker ([`check`]): A4 portrait, margins top/bottom 20 mm, left 30 mm, right 15 mm,
//! Times New Roman; quốc hiệu and tiêu ngữ, issuing body, number/symbol, place and date
//! in a borderless two-column header block; document type and summary; body;
//! recipients and signature block; page numbers centred in the top margin from the
//! second page. Every text is a placeholder ("[Tên cơ quan, tổ chức]", "Số: …/…").

pub mod check;
pub mod nd30;

use bo_model::{
    Alignment, Block, Border, Borders, Cell, CellProps, Document, HalfPoints, HeaderFooter,
    LineSpacing, PageSetup, ParaProps, Paragraph, Row, RowProps, Run, RunContent, RunProps,
    Section, Table, TableProps, TableWidth, Twips,
};
use nd30::{Component, BODY, PAGE};

/// Template ids offered by "File → New from template" (also the file stems of the
/// committed copies in `assets/templates/doc/`).
pub const TEMPLATES: &[&str] = &[
    "cong-van",
    "quyet-dinh",
    "to-trinh",
    "bien-ban",
    "hop-dong",
    "bao-cao",
    "giay-moi",
    "ke-hoach",
];

/// Build a template document.
pub fn build(id: &str) -> Option<Document> {
    let blocks = match id {
        "cong-van" => cong_van(),
        "quyet-dinh" => quyet_dinh(),
        "to-trinh" => to_trinh(),
        "bien-ban" => bien_ban(),
        "hop-dong" => hop_dong(),
        "bao-cao" => bao_cao(),
        "giay-moi" => giay_moi(),
        "ke-hoach" => ke_hoach(),
        _ => return None,
    };
    Some(document(blocks))
}

impl crate::Engine {
    /// `checkAdminFormat`: the deviations of the open document from the decree;
    /// `fixAdminFormat`: correct the fixable ones (one undoable edit), then check again.
    pub(crate) fn template_command(
        &mut self,
        cmd: &crate::edit::Command,
    ) -> crate::Result<Option<crate::edit::EditResult>> {
        let (dirty, error) = match cmd {
            crate::edit::Command::CheckAdminFormat => (Vec::new(), None),
            crate::edit::Command::FixAdminFormat { ids } => {
                let s = self
                    .session
                    .as_mut()
                    .ok_or(crate::EngineError::NoDocument)?;
                let sections = check::fix(s.doc(), ids.as_deref());
                let comments = s.doc().comments.clone();
                let error = s.replace_content(sections, comments).err();
                (self.relayout()?, error.map(|e| e.to_string()))
            }
            _ => return Ok(None),
        };
        let report = self.document().map(check::check);
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        r.admin_format = report.map(Box::new);
        if matches!(cmd, crate::edit::Command::FixAdminFormat { .. }) && r.collab.is_none() {
            r.collab = self.collab_after_edit();
        }
        Ok(Some(r))
    }
}

// ---- page geometry -------------------------------------------------------------

/// A4 portrait (ECMA-376 Part 1, §17.6.13 pgSz) with the preferred margins (§17.6.11 pgMar).
fn page() -> PageSetup {
    let [top, bottom, left, right] = PAGE.preferred.map(nd30::mm_to_twips);
    PageSetup {
        width: Twips(11_906),
        height: Twips(16_838),
        margin_top: Twips(top),
        margin_bottom: Twips(bottom),
        margin_left: Twips(left),
        margin_right: Twips(right),
        header: Twips(567),
        footer: Twips(567),
    }
}

/// Width of the text column, twips.
fn text_width() -> i32 {
    let p = page();
    p.width.0 - p.margin_left.0 - p.margin_right.0
}

/// Header block columns (issuing body | national title), twips.
const HEADER_LEFT: i32 = 3_686;
/// Signature block columns (recipients | signer), twips.
const SIGN_LEFT: i32 = 4_394;

/// Width of the motto "Độc lập - Tự do - Hạnh phúc" at 13 pt bold in Times New Roman
/// metrics (measured with the bundled metric-compatible Tinos), twips; the line below
/// the motto has this length.
const MOTTO_WIDTH: i32 = 3_180;
/// Line below the issuing body (1/3–1/2 of its placeholder, 154 pt), twips.
const ISSUER_RULE: i32 = 1_247;
/// Line below the summary (1/3–1/2 of a typical one-line summary), twips.
const SUMMARY_RULE: i32 = 1_701;

// ---- paragraphs ----------------------------------------------------------------

/// §17.3.2 run properties of a component: sz (half-points), b, i.
fn run_props(size: u32, bold: bool, italic: bool) -> RunProps {
    RunProps {
        size: Some(HalfPoints(size)),
        bold: bold.then_some(true),
        italic: italic.then_some(true),
        ..RunProps::default()
    }
}

fn component_props(c: &Component) -> RunProps {
    run_props(c.preferred, c.bold == Some(true), c.italic == Some(true))
}

/// §17.3.1 paragraph: jc, spacing before/after (twips).
fn para(runs: Vec<Run>, align: Alignment, before: i32) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(align),
            space_before: Some(Twips(before)),
            space_after: Some(Twips(0)),
            ..ParaProps::default()
        },
        runs,
    ))
}

/// One line of a component, in its preferred size and style.
fn line(c: &Component, text: &str) -> Block {
    para(
        vec![Run::text(text, component_props(c))],
        c.align.unwrap_or(Alignment::Center),
        0,
    )
}

/// Like [`line`] with space before (twips).
fn line_after(c: &Component, text: &str, before: i32) -> Block {
    para(
        vec![Run::text(text, component_props(c))],
        c.align.unwrap_or(Alignment::Center),
        before,
    )
}

/// A centred line outside the component table (authority, "QUYẾT ĐỊNH:", "(Chữ ký)").
fn centered(text: &str, size: u32, bold: bool, italic: bool) -> Block {
    para(
        vec![Run::text(text, run_props(size, bold, italic))],
        Alignment::Center,
        0,
    )
}

fn empty(size: u32) -> Block {
    para(
        vec![Run::text("", run_props(size, false, false))],
        Alignment::Center,
        0,
    )
}

/// Solid horizontal line of `len` twips centred in a container `width` twips wide: an
/// empty paragraph with a top border (§17.3.1.24 pBdr, §17.3.1.42 top) between indents
/// (§17.3.1.12 ind), 4 pt below the line above, 1 pt tall (§17.3.1.33 spacing exact).
fn rule(width: i32, len: i32) -> Block {
    let side = ((width - len) / 2).max(0);
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Center),
            indent_left: Some(Twips(side)),
            indent_right: Some(Twips(side)),
            space_before: Some(Twips(80)),
            space_after: Some(Twips(0)),
            line_spacing: Some(LineSpacing::Exact(Twips(20))),
            borders: Some(Borders {
                top: Some(Border::single(6)),
                ..Borders::default()
            }),
            ..ParaProps::default()
        },
        Vec::new(),
    ))
}

/// Body text segment: (text, bold, italic).
type Seg<'a> = (&'a str, bool, bool);

/// Body paragraph (nội dung): justified, first line indented 1 cm, 6 pt after, 1.15
/// lines, 14 pt (Phụ lục I: canh đều 2 lề, lùi đầu dòng 1–1,27 cm, cách đoạn ≥ 6 pt,
/// dòng đơn đến 1,5 lines, cỡ chữ 13–14).
fn body_runs(segs: &[Seg<'_>]) -> Block {
    let runs = segs
        .iter()
        .map(|(t, b, i)| Run::text(*t, run_props(BODY.preferred, *b, *i)))
        .collect();
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(BODY.align),
            indent_first_line: Some(Twips(BODY.preferred_first_line)),
            space_before: Some(Twips(0)),
            space_after: Some(Twips(BODY.preferred_after)),
            line_spacing: Some(LineSpacing::Auto(BODY.preferred_line)),
            ..ParaProps::default()
        },
        runs,
    ))
}

fn body(text: &str) -> Block {
    body_runs(&[(text, false, false)])
}

fn body_bold(text: &str) -> Block {
    body_runs(&[(text, true, false)])
}

fn body_italic(text: &str) -> Block {
    body_runs(&[(text, false, true)])
}

/// "Điều n. Title" with the article label in bold.
fn article(label: &str, text: &str) -> Block {
    body_runs(&[(label, true, false), (text, false, false)])
}

// ---- blocks ----------------------------------------------------------------------

fn nil() -> Border {
    Border {
        style: "nil".into(),
        size: 0,
        color: None,
        space: 0,
    }
}

/// A one-row layout table without borders (§17.4.39 tblBorders = nil) or cell margins
/// (§17.4.42 tblCellMar), one cell per column.
fn columns(widths: &[i32], cells: Vec<Vec<Block>>) -> Block {
    let cells = widths
        .iter()
        .zip(cells)
        .map(|(&w, blocks)| Cell {
            props: CellProps {
                width: Some(TableWidth::Dxa(Twips(w))),
                ..CellProps::default()
            },
            blocks,
        })
        .collect();
    Block::Table(Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(Twips(widths.iter().sum()))),
            borders: Some(Borders::all(&nil())),
            fixed_layout: true,
            cell_margin: Some((Twips(0), Twips(0))),
            ..TableProps::default()
        },
        grid: widths.iter().copied().map(Twips).collect(),
        grid_change: None,
        rows: vec![Row {
            props: RowProps::default(),
            cells,
        }],
        src: None,
    })
}

/// Header block (Mẫu 1.1 ô 1–4, 5b): issuing body, number and symbol (and the "V/v"
/// summary of a công văn) on the left; national title, motto, place and date on the
/// right.
fn header(number: &str, summary: Option<&str>) -> Block {
    let right_w = text_width() - HEADER_LEFT;
    let mut left = vec![
        line(&nd30::CO_QUAN_CHU_QUAN, "[TÊN CQ, TC CHỦ QUẢN]"),
        line(&nd30::CO_QUAN, "[TÊN CƠ QUAN, TỔ CHỨC]"),
        rule(HEADER_LEFT, ISSUER_RULE),
        line_after(&nd30::SO_KY_HIEU, number, 160),
    ];
    if let Some(s) = summary {
        left.push(line_after(&nd30::TRICH_YEU_CONG_VAN, s, 60));
    }
    let right = vec![
        line(&nd30::QUOC_HIEU, "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"),
        line(&nd30::TIEU_NGU, "Độc lập - Tự do - Hạnh phúc"),
        rule(right_w, MOTTO_WIDTH),
        line_after(&nd30::DIA_DANH, "[Địa danh], ngày … tháng … năm 20…", 160),
    ];
    columns(&[HEADER_LEFT, right_w], vec![left, right])
}

/// Document type and summary (ô 5a) with the line below the summary.
fn title(kind: &str, summary: &str) -> Vec<Block> {
    vec![
        empty(BODY.preferred),
        line_after(&nd30::TEN_LOAI, kind, 120),
        line(&nd30::TRICH_YEU, summary),
        rule(text_width(), SUMMARY_RULE),
        empty(BODY.preferred),
    ]
}

/// "Kính gửi: …" (centred, 14 pt).
fn kinh_gui(to: &str) -> Block {
    para(
        vec![Run::text(
            format!("Kính gửi: {to}"),
            component_props(&nd30::KINH_GUI),
        )],
        Alignment::Center,
        120,
    )
}

/// Signer block (ô 7a–7c): authority and title (one line per entry), signature/seal
/// note, name; level with "Nơi nhận:".
fn signer(titles: &[&str], note: &str) -> Vec<Block> {
    let mut v: Vec<Block> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| line_after(&nd30::CHUC_VU, t, if i == 0 { 120 } else { 0 }))
        .collect();
    v.extend([
        centered(note, 26, false, true),
        empty(BODY.preferred),
        empty(BODY.preferred),
        empty(BODY.preferred),
        line(&nd30::HO_TEN, "[Họ và tên]"),
    ]);
    v
}

/// Recipients (ô 9b).
fn noi_nhan(first: &str) -> Vec<Block> {
    vec![
        line_after(&nd30::NOI_NHAN, "Nơi nhận:", 120),
        line(&nd30::NOI_NHAN_LIST, first),
        line(&nd30::NOI_NHAN_LIST, "- [Tên cơ quan, tổ chức nhận];"),
        line(&nd30::NOI_NHAN_LIST, "- Lưu: VT, [đơn vị soạn thảo]."),
    ]
}

/// Recipients on the left, signer on the right.
fn closing(first_recipient: &str) -> Vec<Block> {
    let sign = signer(
        &["[QUYỀN HẠN KÝ]", "[CHỨC VỤ CỦA NGƯỜI KÝ]"],
        "(Chữ ký, dấu)",
    );
    vec![
        columns(
            &[SIGN_LEFT, text_width() - SIGN_LEFT],
            vec![noi_nhan(first_recipient), sign],
        ),
        empty(BODY.preferred),
    ]
}

/// Two signers side by side (biên bản, hợp đồng).
fn two_signers(left: (&str, &str), right: (&str, &str)) -> Vec<Block> {
    let (left, right) = ((&[left.0][..], left.1), (&[right.0][..], right.1));
    let half = text_width() / 2;
    vec![
        columns(
            &[half, text_width() - half],
            vec![signer(left.0, left.1), signer(right.0, right.1)],
        ),
        empty(BODY.preferred),
    ]
}

/// The document around the blocks: page, page number header, default font.
fn document(blocks: Vec<Block>) -> Document {
    // Page number centred in the top margin, not on the first page (§17.10.6 titlePg,
    // §17.16.5.43 PAGE): Phụ lục I, "số trang … canh giữa theo chiều ngang trong phần
    // lề trên, không hiển thị số trang thứ nhất".
    let number = para(
        vec![Run {
            props: component_props(&nd30::SO_TRANG),
            content: vec![RunContent::Field {
                instr: "PAGE".into(),
                cached: "2".into(),
                locked: false,
            }],
        }],
        Alignment::Center,
        0,
    );
    let headers = HeaderFooter {
        default: Some(vec![number]),
        first: Some(vec![empty(nd30::SO_TRANG.preferred)]),
        even: None,
    };
    Document {
        sections: vec![Section {
            page: page(),
            headers,
            title_page: true,
            ..Section::new(blocks)
        }],
        // §17.7.5.2 rPrDefault: rFonts + sz.
        default_run: RunProps {
            font: Some(nd30::FONT.into()),
            size: Some(HalfPoints(BODY.preferred)),
            ..RunProps::default()
        },
        ..Document::default()
    }
}

// ---- the templates ---------------------------------------------------------------

/// Công văn (Phụ lục III, Mẫu 1.5): no type name; "V/v" summary below the number.
fn cong_van() -> Vec<Block> {
    let mut b = vec![
        header("Số: …/…-…", Some("V/v [trích yếu nội dung công văn]")),
        empty(BODY.preferred),
        kinh_gui("[Tên cơ quan, tổ chức nhận văn bản]."),
        empty(BODY.preferred),
        body("[Nội dung công văn: trình bày bằng chữ in thường, canh đều hai lề, lùi đầu dòng 1 cm; khoảng cách giữa các đoạn văn tối thiểu 6 pt; khoảng cách giữa các dòng từ dòng đơn đến 1,5 dòng.]"),
        body("[Nội dung …]"),
        body("[Tên cơ quan, tổ chức] đề nghị [tên cơ quan, tổ chức nhận văn bản] xem xét, giải quyết./."),
    ];
    b.extend(closing("- Như trên;"));
    b
}

/// Quyết định (cá biệt, ban hành trực tiếp).
fn quyet_dinh() -> Vec<Block> {
    let mut b = vec![header("Số: …/QĐ-…", None)];
    b.extend(title(
        "QUYẾT ĐỊNH",
        "Về việc [trích yếu nội dung quyết định]",
    ));
    b.extend([
        centered("[THẨM QUYỀN BAN HÀNH]", BODY.preferred, true, false),
        empty(BODY.preferred),
        body_italic("Căn cứ [văn bản quy định chức năng, nhiệm vụ, quyền hạn của cơ quan, tổ chức ban hành];"),
        body_italic("Căn cứ [văn bản làm căn cứ ban hành];"),
        body_italic("Theo đề nghị của [người đứng đầu đơn vị trình]."),
        centered("QUYẾT ĐỊNH:", BODY.preferred, true, false),
        empty(BODY.preferred),
        article("Điều 1. ", "[Nội dung quyết định …]"),
        article("Điều 2. ", "Quyết định này có hiệu lực kể từ ngày [ký / ngày … tháng … năm …]."),
        article("Điều 3. ", "[Người đứng đầu các cơ quan, đơn vị, cá nhân có liên quan] chịu trách nhiệm thi hành Quyết định này./."),
    ]);
    b.extend(closing("- Như Điều 3;"));
    b
}

/// Tờ trình.
fn to_trinh() -> Vec<Block> {
    let mut b = vec![header("Số: …/TTr-…", None)];
    b.extend(title("TỜ TRÌNH", "Về việc [trích yếu nội dung trình]"));
    b.extend([
        kinh_gui("[Cơ quan, người có thẩm quyền xem xét, quyết định]"),
        empty(BODY.preferred),
        body("[Tên cơ quan, tổ chức] trình [cấp có thẩm quyền] xem xét, quyết định [vấn đề trình] như sau:"),
        body_bold("1. Sự cần thiết"),
        body("[Cơ sở pháp lý, cơ sở thực tiễn …]"),
        body_bold("2. Nội dung đề xuất"),
        body("[Nội dung …]"),
        body_bold("3. Kiến nghị"),
        body("Kính trình [cấp có thẩm quyền] xem xét, quyết định./."),
    ]);
    b.extend(closing("- Như trên;"));
    b
}

/// Biên bản: secretary and chair sign side by side.
fn bien_ban() -> Vec<Block> {
    let mut b = vec![header("Số: …/BB-…", None)];
    b.extend(title("BIÊN BẢN", "[Trích yếu nội dung cuộc họp, sự kiện]"));
    b.extend([
        body("Thời gian bắt đầu: … giờ … phút, ngày … tháng … năm …"),
        body("Địa điểm: [địa điểm tổ chức]"),
        body("Thành phần tham dự: [họ tên, chức vụ của những người tham dự]"),
        body("Chủ trì (chủ tọa): [họ tên, chức vụ]; thư ký (người ghi biên bản): [họ tên, chức vụ]."),
        body_bold("Nội dung (theo diễn biến cuộc họp, sự kiện):"),
        body("[Nội dung …]"),
        body("Cuộc họp (sự kiện) kết thúc vào … giờ … phút, ngày … tháng … năm …; biên bản đã được đọc lại cho những người tham dự cùng nghe và thông qua./."),
    ]);
    b.extend(two_signers(
        ("THƯ KÝ", "(Chữ ký)"),
        ("CHỦ TỌA", "(Chữ ký, dấu (nếu có))"),
    ));
    b
}

/// Hợp đồng: parties, articles, both representatives sign.
fn hop_dong() -> Vec<Block> {
    let mut b = vec![header("Số: …/HĐ-…", None)];
    b.extend(title("HỢP ĐỒNG", "[Tên loại hợp đồng]"));
    b.extend([
        body_italic("Căn cứ [văn bản quy phạm pháp luật làm căn cứ ký kết hợp đồng];"),
        body_italic("Căn cứ [nhu cầu và khả năng của các bên]."),
        body("Hôm nay, ngày … tháng … năm …, tại [địa điểm], chúng tôi gồm:"),
        body_bold("Bên A: [Tên cơ quan, tổ chức]"),
        body("Địa chỉ: …; điện thoại: …; mã số thuế: …"),
        body("Đại diện: [họ và tên]; chức vụ: …"),
        body_bold("Bên B: [Tên cơ quan, tổ chức, cá nhân]"),
        body("Địa chỉ: …; điện thoại: …; mã số thuế: …"),
        body("Đại diện: [họ và tên]; chức vụ: …"),
        body("Hai bên thống nhất ký kết hợp đồng với các điều khoản sau:"),
        article("Điều 1. ", "Nội dung hợp đồng: […]"),
        article("Điều 2. ", "Giá trị hợp đồng và phương thức thanh toán: […]"),
        article("Điều 3. ", "Quyền và nghĩa vụ của các bên: […]"),
        article("Điều 4. ", "Điều khoản chung: hợp đồng có hiệu lực kể từ ngày ký, được lập thành … bản có giá trị pháp lý như nhau, mỗi bên giữ … bản./."),
    ]);
    b.extend(two_signers(
        ("ĐẠI DIỆN BÊN A", "(Chữ ký, dấu)"),
        ("ĐẠI DIỆN BÊN B", "(Chữ ký, dấu)"),
    ));
    b
}

/// Báo cáo.
fn bao_cao() -> Vec<Block> {
    let mut b = vec![header("Số: …/BC-…", None)];
    b.extend(title("BÁO CÁO", "[Trích yếu nội dung báo cáo]"));
    b.extend([
        body_bold("I. [KẾT QUẢ THỰC HIỆN]"),
        body("1. […]"),
        body("2. […]"),
        body_bold("II. [TỒN TẠI, HẠN CHẾ VÀ NGUYÊN NHÂN]"),
        body("[…]"),
        body_bold("III. [PHƯƠNG HƯỚNG, NHIỆM VỤ THỜI GIAN TỚI]"),
        body("[…]"),
        body_bold("IV. [KIẾN NGHỊ, ĐỀ XUẤT]"),
        body("[…]"),
        body("Trên đây là báo cáo [trích yếu] của [tên cơ quan, tổ chức]./."),
    ]);
    b.extend(closing("- [Cơ quan nhận báo cáo];"));
    b
}

/// Giấy mời.
fn giay_moi() -> Vec<Block> {
    let mut b = vec![header("Số: …/GM-…", None)];
    b.extend(title(
        "GIẤY MỜI",
        "[Trích yếu nội dung cuộc họp, hội nghị, sự kiện]",
    ));
    b.extend([
        body(
            "[Tên cơ quan, tổ chức] trân trọng kính mời: [tên cơ quan, tổ chức, cá nhân được mời].",
        ),
        body("Tới dự: [tên cuộc họp, hội nghị, sự kiện]."),
        body("Thời gian: … giờ … phút, ngày … tháng … năm …"),
        body("Địa điểm: [địa điểm tổ chức]."),
        body("[Đề nghị đối với người được mời: tài liệu mang theo, đầu mối liên hệ …]./."),
    ]);
    b.extend(closing("- Như trên;"));
    b
}

/// Kế hoạch.
fn ke_hoach() -> Vec<Block> {
    let mut b = vec![header("Số: …/KH-…", None)];
    b.extend(title("KẾ HOẠCH", "[Trích yếu nội dung kế hoạch]"));
    b.extend([
        body("Thực hiện [văn bản chỉ đạo], [tên cơ quan, tổ chức] ban hành Kế hoạch [trích yếu] như sau:"),
        body_bold("I. MỤC ĐÍCH, YÊU CẦU"),
        body("1. Mục đích: […]"),
        body("2. Yêu cầu: […]"),
        body_bold("II. NỘI DUNG VÀ TIẾN ĐỘ THỰC HIỆN"),
        body("1. […]"),
        body("2. […]"),
        body_bold("III. TỔ CHỨC THỰC HIỆN"),
        body("1. [Đơn vị chủ trì]: […]"),
        body("2. [Đơn vị phối hợp]: […]"),
        body("Trong quá trình thực hiện, nếu có vướng mắc, các đơn vị phản ánh về [đơn vị đầu mối] để tổng hợp, báo cáo [người có thẩm quyền] xem xét, quyết định./."),
    ]);
    b.extend(closing("- [Các đơn vị thực hiện];"));
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_follow_the_decree() {
        for id in TEMPLATES {
            let d = build(id).expect("template");
            let s = &d.sections[0];
            assert_eq!((s.page.width.0, s.page.height.0), (11_906, 16_838));
            let mm = |t: Twips| nd30::twips_to_mm(t.0);
            assert!(PAGE.top.contains(mm(s.page.margin_top)), "{id}");
            assert!(PAGE.bottom.contains(mm(s.page.margin_bottom)), "{id}");
            assert!(PAGE.left.contains(mm(s.page.margin_left)), "{id}");
            assert!(PAGE.right.contains(mm(s.page.margin_right)), "{id}");
            assert!(s.title_page);
            let text = d.plain_text();
            for part in [
                "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM",
                "Độc lập - Tự do - Hạnh phúc",
                "Số:",
            ] {
                assert!(text.contains(part), "{id}: {part}");
            }
        }
        assert!(build("nope").is_none());
    }
}
