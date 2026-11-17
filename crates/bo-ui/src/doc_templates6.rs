//! BlueOffice's own document templates offered by File › New (parity round 6). Each is an
//! HTML fragment pasted into a new blank document (through `bo-html`, so headings, alignment,
//! bold text and tables become document formatting). Layouts and wording are BlueOffice's
//! own; the official letter follows the Vietnamese administrative document layout of Decree
//! 30/2020/ND-CP (agency and national motto side by side, number, place and date, title,
//! body, recipients and signature).

/// HTML of template `id` (`letter`, `report`, `minutes`, `resume`, `memo`).
pub(crate) fn html(id: &str) -> Option<&'static str> {
    Some(match id {
        "letter" => LETTER,
        "report" => REPORT,
        "minutes" => MINUTES,
        "resume" => RESUME,
        "memo" => MEMO,
        _ => return None,
    })
}

const LETTER: &str = r#"<table style="width:100%;border-collapse:collapse">
<tr><td style="width:40%;text-align:center;vertical-align:top"><p style="text-align:center"><b>TÊN CƠ QUAN, TỔ CHỨC</b></p><p style="text-align:center">Số: ..../CV-....</p><p style="text-align:center"><i>V/v ....................</i></p></td>
<td style="width:60%;text-align:center;vertical-align:top"><p style="text-align:center"><b>CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM</b></p><p style="text-align:center"><b>Độc lập - Tự do - Hạnh phúc</b></p><p style="text-align:right"><i>Hà Nội, ngày .... tháng .... năm ....</i></p></td></tr>
</table>
<p style="text-align:center">Kính gửi: ....................................</p>
<p style="text-align:justify">Nội dung công văn ............................................................................................................................................</p>
<p style="text-align:justify">............................................................................................................................................................................</p>
<table style="width:100%;border-collapse:collapse">
<tr><td style="width:50%;vertical-align:top"><p><b><i>Nơi nhận:</i></b></p><p>- Như trên;</p><p>- Lưu: VT, ....</p></td>
<td style="width:50%;vertical-align:top"><p style="text-align:center"><b>THỦ TRƯỞNG CƠ QUAN</b></p><p style="text-align:center"><i>(Ký, ghi rõ họ tên)</i></p></td></tr>
</table>"#;

const REPORT: &str = r"<h1>Report Title</h1>
<p><i>Subtitle or project name — Date</i></p>
<h2>Summary</h2>
<p>Start with the main points of the report in a few sentences, so readers know what it covers and what it concludes.</p>
<h2>Background</h2>
<p>Describe the situation, the questions the report answers and the sources it uses.</p>
<h2>Findings</h2>
<ul><li>First finding</li><li>Second finding</li><li>Third finding</li></ul>
<h2>Recommendations</h2>
<p>List the actions you propose, who should take them and by when.</p>";

const MINUTES: &str = r#"<h1>Meeting Minutes</h1>
<table style="width:100%;border-collapse:collapse">
<tr><td style="width:25%"><b>Date</b></td><td>....</td></tr>
<tr><td><b>Time</b></td><td>....</td></tr>
<tr><td><b>Location</b></td><td>....</td></tr>
<tr><td><b>Chair</b></td><td>....</td></tr>
<tr><td><b>Attendees</b></td><td>....</td></tr>
</table>
<h2>Agenda</h2>
<ol><li>Opening</li><li>Review of previous actions</li><li>Discussion</li><li>Next steps</li></ol>
<h2>Decisions</h2>
<p>Record each decision with its reasons.</p>
<h2>Action items</h2>
<table style="width:100%;border-collapse:collapse">
<tr><td><b>Action</b></td><td><b>Owner</b></td><td><b>Due</b></td></tr>
<tr><td>....</td><td>....</td><td>....</td></tr>
</table>"#;

const RESUME: &str = r"<h1>Your Name</h1>
<p>Address · Phone · Email</p>
<h2>Objective</h2>
<p>One or two sentences about the role you are looking for and what you bring to it.</p>
<h2>Experience</h2>
<p><b>Job Title</b> — Company, City (Year – Year)</p>
<ul><li>Main responsibility or achievement</li><li>Another result, with numbers when you can</li></ul>
<h2>Education</h2>
<p><b>Degree</b> — School (Year)</p>
<h2>Skills</h2>
<ul><li>Skill</li><li>Skill</li><li>Skill</li></ul>";

const MEMO: &str = r#"<h1>MEMO</h1>
<table style="width:100%;border-collapse:collapse">
<tr><td style="width:18%"><b>To:</b></td><td>....</td></tr>
<tr><td><b>From:</b></td><td>....</td></tr>
<tr><td><b>Date:</b></td><td>....</td></tr>
<tr><td><b>Subject:</b></td><td>....</td></tr>
</table>
<p>Write the purpose of the memo in the first paragraph, then the details and what you ask the readers to do.</p>"#;

#[cfg(test)]
mod tests {
    #[test]
    fn every_template_has_html() {
        for (id, ..) in crate::doc_backstage6::TEMPLATES {
            assert!(super::html(id).is_some_and(|h| h.contains('<')), "{id}");
        }
        assert!(super::html("nope").is_none());
    }
}
