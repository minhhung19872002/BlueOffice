//! Presentation rules of Vietnamese administrative documents (thể thức và kỹ thuật trình
//! bày văn bản hành chính), Nghị định 30/2020/NĐ-CP, Phụ lục I — encoded as data.
//!
//! The same table drives the built-in templates (they use each component's preferred
//! size and style) and the format checker ([`super::check`]), so the two can never
//! disagree. Sizes are in half-points (ECMA-376 Part 1, §17.3.2.38 `w:sz`), lengths in
//! millimetres; the decree's component numbers ("ô số") refer to its layout sample
//! (Mẫu 1.1, vị trí trình bày các thành phần thể thức).

use bo_model::Alignment;

/// Typeface required by the decree (Unicode font of TCVN 6909:2001); renderers may
/// substitute a metric-compatible face (BlueOffice bundles Tinos), but the document
/// itself must name this font.
pub const FONT: &str = "Times New Roman";

/// Inclusive range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range<T> {
    /// Lowest allowed value.
    pub min: T,
    /// Highest allowed value.
    pub max: T,
}

const fn r<T>(min: T, max: T) -> Range<T> {
    Range { min, max }
}

impl<T: PartialOrd + Copy> Range<T> {
    /// `v` lies in the range.
    pub fn contains(&self, v: T) -> bool {
        self.min <= v && v <= self.max
    }
}

/// Paper and margins (Phụ lục I, mục IV: khổ giấy, kiểu trình bày, định lề trang).
#[derive(Debug, Clone, Copy)]
pub struct PageRule {
    /// A4 width, mm.
    pub width: f64,
    /// A4 height, mm (portrait: "theo chiều dài của khổ A4").
    pub height: f64,
    /// Allowed difference of the paper size, mm.
    pub size_tolerance: f64,
    /// Top margin, mm.
    pub top: Range<f64>,
    /// Bottom margin, mm.
    pub bottom: Range<f64>,
    /// Left margin, mm.
    pub left: Range<f64>,
    /// Right margin, mm.
    pub right: Range<f64>,
    /// Margins the templates use, mm (top, bottom, left, right).
    pub preferred: [f64; 4],
}

/// Page rule of the decree.
pub const PAGE: PageRule = PageRule {
    width: 210.0,
    height: 297.0,
    size_tolerance: 1.0,
    top: r(20.0, 25.0),
    bottom: r(20.0, 25.0),
    left: r(30.0, 35.0),
    right: r(15.0, 20.0),
    preferred: [20.0, 20.0, 30.0, 15.0],
};

/// Where a component sits on the first page (Mẫu 1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// Left column of the header block (issuing body, number, "V/v" summary).
    HeaderLeft,
    /// Right column of the header block (national title, motto, place and date).
    HeaderRight,
    /// Below the header block, full text width.
    Body,
    /// Left column of the signature block (recipients).
    SignatureLeft,
    /// Signature column(s): title and name of the signer.
    Signature,
}

impl Area {
    /// Stable name used in reports.
    pub fn name(self) -> &'static str {
        match self {
            Area::HeaderLeft => "headerLeft",
            Area::HeaderRight => "headerRight",
            Area::Body => "body",
            Area::SignatureLeft => "signatureLeft",
            Area::Signature => "signature",
        }
    }
}

/// Letter case of a component's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Any case.
    Any,
    /// All capitals ("chữ in hoa").
    Upper,
}

/// One component of the document format (thành phần thể thức).
#[derive(Debug, Clone, Copy)]
pub struct Component {
    /// Stable id (report ids are `<id>.<property>`).
    pub id: &'static str,
    /// Number of the position in Mẫu 1.1 ("ô số").
    pub cell: &'static str,
    /// Allowed font sizes, half-points.
    pub size: Range<u32>,
    /// Size used by the templates, half-points.
    pub preferred: u32,
    /// Required bold (`None`: either).
    pub bold: Option<bool>,
    /// Required italic (`None`: either).
    pub italic: Option<bool>,
    /// Required letter case.
    pub case: Case,
    /// Required paragraph alignment (`None`: any).
    pub align: Option<Alignment>,
    /// Expected position.
    pub area: Area,
    /// A solid horizontal line is drawn below ("phía dưới có đường kẻ ngang, nét liền").
    pub rule_below: bool,
    /// The component must be present in every administrative document.
    pub required: bool,
}

const fn comp(id: &'static str, cell: &'static str, size: Range<u32>, area: Area) -> Component {
    Component {
        id,
        cell,
        size,
        preferred: size.min,
        bold: None,
        italic: None,
        case: Case::Any,
        align: Some(Alignment::Center),
        area,
        rule_below: false,
        required: false,
    }
}

/// Quốc hiệu: "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", in hoa, 12–13 pt, đứng, đậm.
pub const QUOC_HIEU: Component = Component {
    bold: Some(true),
    italic: Some(false),
    case: Case::Upper,
    required: true,
    ..comp("quocHieu", "1", r(24, 26), Area::HeaderRight)
};

/// Tiêu ngữ: "Độc lập - Tự do - Hạnh phúc", 13–14 pt, đứng, đậm, chữ cái đầu cụm từ viết
/// hoa; đường kẻ ngang nét liền bên dưới, dài bằng dòng chữ.
pub const TIEU_NGU: Component = Component {
    bold: Some(true),
    italic: Some(false),
    rule_below: true,
    required: true,
    ..comp("tieuNgu", "1", r(26, 28), Area::HeaderRight)
};

/// Tên cơ quan, tổ chức chủ quản trực tiếp (nếu có): in hoa, 12–13 pt, đứng, không đậm.
pub const CO_QUAN_CHU_QUAN: Component = Component {
    bold: Some(false),
    italic: Some(false),
    case: Case::Upper,
    ..comp("coQuanChuQuan", "2", r(24, 26), Area::HeaderLeft)
};

/// Tên cơ quan, tổ chức ban hành: in hoa, 12–13 pt, đứng, đậm; đường kẻ ngang bên dưới
/// dài 1/3–1/2 dòng chữ, đặt cân đối.
pub const CO_QUAN: Component = Component {
    bold: Some(true),
    italic: Some(false),
    case: Case::Upper,
    rule_below: true,
    required: true,
    ..comp("coQuan", "2", r(24, 26), Area::HeaderLeft)
};

/// Số, ký hiệu: "Số:" chữ thường, ký hiệu in hoa, 13 pt, đứng; canh giữa dưới tên cơ quan.
pub const SO_KY_HIEU: Component = Component {
    italic: Some(false),
    required: true,
    ..comp("soKyHieu", "3", r(26, 26), Area::HeaderLeft)
};

/// Địa danh và thời gian ban hành: chữ thường, 13–14 pt, nghiêng; canh giữa dưới tiêu ngữ.
pub const DIA_DANH: Component = Component {
    italic: Some(true),
    required: true,
    ..comp("diaDanh", "4", r(26, 28), Area::HeaderRight)
};

/// Tên loại văn bản: in hoa, 13–14 pt, đứng, đậm, canh giữa.
pub const TEN_LOAI: Component = Component {
    bold: Some(true),
    italic: Some(false),
    case: Case::Upper,
    preferred: 28,
    ..comp("tenLoai", "5a", r(26, 28), Area::Body)
};

/// Trích yếu nội dung: chữ thường, 13–14 pt, đứng, đậm, canh giữa dưới tên loại;
/// đường kẻ ngang bên dưới dài 1/3–1/2 dòng chữ.
pub const TRICH_YEU: Component = Component {
    bold: Some(true),
    italic: Some(false),
    rule_below: true,
    preferred: 28,
    ..comp("trichYeu", "5a", r(26, 28), Area::Body)
};

/// Trích yếu của công văn: sau "V/v", chữ thường, 12–13 pt, đứng, canh giữa dưới số và
/// ký hiệu.
pub const TRICH_YEU_CONG_VAN: Component = Component {
    bold: Some(false),
    italic: Some(false),
    ..comp("trichYeuCongVan", "5b", r(24, 26), Area::HeaderLeft)
};

/// "Kính gửi:" and the addressees of a công văn / tờ trình: chữ thường, 14 pt, đứng.
pub const KINH_GUI: Component = Component {
    bold: Some(false),
    italic: Some(false),
    align: None,
    preferred: 28,
    ..comp("kinhGui", "9a", r(26, 28), Area::Body)
};

/// Quyền hạn, chức vụ của người ký: in hoa, 13–14 pt, đứng, đậm.
pub const CHUC_VU: Component = Component {
    bold: Some(true),
    italic: Some(false),
    case: Case::Upper,
    required: true,
    ..comp("chucVu", "7a", r(26, 28), Area::Signature)
};

/// Họ và tên người ký: chữ thường, 13–14 pt, đứng, đậm, canh giữa.
pub const HO_TEN: Component = Component {
    bold: Some(true),
    italic: Some(false),
    required: true,
    ..comp("hoTen", "7b", r(26, 28), Area::Signature)
};

/// "Nơi nhận:": chữ thường, 12 pt, nghiêng, đậm, sát lề trái.
pub const NOI_NHAN: Component = Component {
    bold: Some(true),
    italic: Some(true),
    align: Some(Alignment::Left),
    ..comp("noiNhan", "9b", r(24, 24), Area::SignatureLeft)
};

/// The recipients listed under "Nơi nhận:": chữ thường, 11 pt, đứng.
pub const NOI_NHAN_LIST: Component = Component {
    bold: Some(false),
    italic: Some(false),
    align: Some(Alignment::Left),
    ..comp("noiNhanList", "9b", r(22, 22), Area::SignatureLeft)
};

/// Số trang: 13–14 pt, đứng, canh giữa lề trên, không hiển thị ở trang thứ nhất.
pub const SO_TRANG: Component = Component {
    italic: Some(false),
    ..comp("soTrang", "-", r(26, 28), Area::Body)
};

/// Every component, in the order of Mẫu 1.1.
pub const COMPONENTS: &[Component] = &[
    QUOC_HIEU,
    TIEU_NGU,
    CO_QUAN_CHU_QUAN,
    CO_QUAN,
    SO_KY_HIEU,
    DIA_DANH,
    TEN_LOAI,
    TRICH_YEU,
    TRICH_YEU_CONG_VAN,
    KINH_GUI,
    CHUC_VU,
    HO_TEN,
    NOI_NHAN,
    NOI_NHAN_LIST,
    SO_TRANG,
];

/// Body text (nội dung, ô 6).
#[derive(Debug, Clone, Copy)]
pub struct BodyRule {
    /// Font size, half-points ("cỡ chữ từ 13 đến 14").
    pub size: Range<u32>,
    /// Size used by the templates.
    pub preferred: u32,
    /// "Canh đều 2 lề".
    pub align: Alignment,
    /// First-line indent, twips ("lùi đầu dòng từ 1 cm đến 1,27 cm"; 567–720 twips).
    pub first_line: Range<i32>,
    /// Minimum space between paragraphs, twips ("tối thiểu là 6 pt").
    pub min_paragraph_spacing: i32,
    /// Line spacing as a multiple of single spacing, 240ths ("tối thiểu là dòng đơn …
    /// tối đa là 1,5 lines").
    pub line_auto: Range<u32>,
    /// Exact / at-least line height, twips ("từ 15 pt (exactly) trở lên").
    pub line_exact: Range<i32>,
    /// Line spacing the templates use (240ths).
    pub preferred_line: u32,
    /// First-line indent the templates use, twips (1 cm).
    pub preferred_first_line: i32,
    /// Space after a paragraph in the templates, twips (6 pt).
    pub preferred_after: i32,
}

/// Body rule of the decree.
pub const BODY: BodyRule = BodyRule {
    size: r(26, 28),
    preferred: 28,
    align: Alignment::Justify,
    first_line: r(567, 720),
    min_paragraph_spacing: 120,
    line_auto: r(240, 360),
    line_exact: r(300, 540),
    preferred_line: 276,
    preferred_first_line: 567,
    preferred_after: 120,
};

/// Document type names (tên loại, Điều 7 and Phụ lục III) recognised by the checker.
pub const TYPE_NAMES: &[&str] = &[
    "NGHỊ QUYẾT",
    "QUYẾT ĐỊNH",
    "CHỈ THỊ",
    "QUY CHẾ",
    "QUY ĐỊNH",
    "THÔNG CÁO",
    "THÔNG BÁO",
    "HƯỚNG DẪN",
    "CHƯƠNG TRÌNH",
    "KẾ HOẠCH",
    "PHƯƠNG ÁN",
    "ĐỀ ÁN",
    "DỰ ÁN",
    "BÁO CÁO",
    "BIÊN BẢN",
    "TỜ TRÌNH",
    "HỢP ĐỒNG",
    "CÔNG ĐIỆN",
    "BẢN GHI NHỚ",
    "BẢN THỎA THUẬN",
    "GIẤY ỦY QUYỀN",
    "GIẤY MỜI",
    "GIẤY GIỚI THIỆU",
    "GIẤY NGHỈ PHÉP",
    "PHIẾU GỬI",
    "PHIẾU CHUYỂN",
    "PHIẾU BÁO",
];

/// Millimetres → twips (1 mm = 1440 / 25.4 twips), rounded up so that a minimum
/// stays satisfied.
pub fn mm_to_twips(mm: f64) -> i32 {
    (mm * 1440.0 / 25.4 - 1e-6).ceil() as i32
}

/// Twips → millimetres.
pub fn twips_to_mm(t: i32) -> f64 {
    f64::from(t) * 25.4 / 1440.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_values_satisfy_the_rules() {
        for c in COMPONENTS {
            assert!(c.size.contains(c.preferred), "{}", c.id);
        }
        assert!(BODY.size.contains(BODY.preferred));
        assert!(BODY.first_line.contains(BODY.preferred_first_line));
        assert!(BODY.line_auto.contains(BODY.preferred_line));
        let [t, b, l, rt] = PAGE.preferred;
        assert!(PAGE.top.contains(t) && PAGE.bottom.contains(b));
        assert!(PAGE.left.contains(l) && PAGE.right.contains(rt));
        assert_eq!(mm_to_twips(20.0), 1134);
        assert_eq!(mm_to_twips(30.0), 1701);
        assert_eq!(mm_to_twips(15.0), 851);
    }
}
