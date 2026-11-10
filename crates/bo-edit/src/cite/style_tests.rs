//! Reference styles: table-driven tests, style × source type. The expected strings are
//! derived from the public style guides named in [`super::style`]; the rule applied is
//! stated next to each expectation. Italic stretches are written between asterisks.
#![allow(clippy::unwrap_used)]

use super::source::{Contributor, Person, Source};
use super::style::{Citation, CiteItem, Formatter, Rich, StyleId};

fn people(role: &str, names: &[(&str, &str)]) -> Contributor {
    Contributor {
        role: role.into(),
        corporate: None,
        people: names.iter().map(|(l, f)| Person::new(l, f)).collect(),
    }
}

fn body(role: &str, name: &str) -> Contributor {
    Contributor {
        role: role.into(),
        corporate: Some(name.into()),
        people: Vec::new(),
    }
}

fn source(tag: &str, kind: &str, who: Vec<Contributor>, fields: &[(&str, &str)]) -> Source {
    let mut s = Source {
        tag: tag.into(),
        kind: kind.into(),
        contributors: who,
        ..Source::default()
    };
    for (k, v) in fields {
        s.set(k, v);
    }
    s
}

/// One source of each kind the styles tell apart.
fn samples() -> Vec<Source> {
    vec![
        source(
            "book",
            "Book",
            vec![people("Author", &[("Smith", "John K."), ("Doe", "Jane")])],
            &[
                ("Title", "Research methods"),
                ("Year", "2020"),
                ("Edition", "2"),
                ("Publisher", "Blue Press"),
                ("City", "Hanoi"),
            ],
        ),
        source(
            "chap",
            "BookSection",
            vec![
                people("Author", &[("Lee", "Anna")]),
                people("Editor", &[("Brown", "Mark")]),
            ],
            &[
                ("Title", "Sampling"),
                ("BookTitle", "Handbook of statistics"),
                ("Year", "2019"),
                ("Pages", "45-67"),
                ("Publisher", "Blue Press"),
                ("City", "London"),
            ],
        ),
        source(
            "art",
            "JournalArticle",
            vec![people(
                "Author",
                &[("Nguyen", "Minh T."), ("Smith", "John K."), ("Tran", "Lan")],
            )],
            &[
                ("Title", "Deep learning for text"),
                ("JournalName", "Journal of Data"),
                ("Year", "2021"),
                ("Volume", "12"),
                ("Issue", "3"),
                ("Pages", "100-120"),
                ("DOI", "10.1000/jd.2021.5"),
            ],
        ),
        source(
            "conf",
            "ConferenceProceedings",
            vec![people("Author", &[("Garcia", "Luis")])],
            &[
                ("Title", "Fast parsing"),
                (
                    "ConferenceName",
                    "Proceedings of the International Conference on Parsing",
                ),
                ("Year", "2018"),
                ("Pages", "10-15"),
                ("Publisher", "ACM"),
                ("City", "Paris"),
            ],
        ),
        source(
            "rep",
            "Report",
            vec![body("Author", "World Health Organization")],
            &[
                ("Title", "Global report"),
                ("Year", "2022"),
                ("Publisher", "WHO Press"),
                ("City", "Geneva"),
            ],
        ),
        source(
            "web",
            "InternetSite",
            vec![body("Author", "Blue Docs Team")],
            &[
                ("Title", "Getting started"),
                ("InternetSiteTitle", "Blue Docs"),
                ("Year", "2023"),
                ("Month", "5"),
                ("Day", "4"),
                ("URL", "https://example.org/start"),
                ("YearAccessed", "2024"),
                ("MonthAccessed", "1"),
                ("DayAccessed", "15"),
            ],
        ),
        source(
            "thes",
            "Report",
            vec![people("Author", &[("Pham", "Hoa")])],
            &[
                ("Title", "Graph algorithms"),
                ("ThesisType", "PhD thesis"),
                ("Institution", "Hanoi University"),
                ("Year", "2017"),
                ("City", "Hanoi"),
            ],
        ),
        source(
            "law",
            "Case",
            vec![],
            &[
                ("Title", "Nghị định quy định về bảo vệ dữ liệu cá nhân"),
                ("CaseNumber", "13/2023/NĐ-CP"),
                ("Court", "Chính phủ"),
                ("Year", "2023"),
                ("Month", "4"),
                ("Day", "17"),
            ],
        ),
    ]
}

const TAGS: [&str; 8] = ["book", "chap", "art", "conf", "rep", "web", "thes", "law"];

fn cite(tag: &str) -> Citation {
    Citation {
        items: vec![CiteItem {
            tag: tag.into(),
            ..CiteItem::default()
        }],
        lcid: None,
    }
}

fn cite_pages(tag: &str, pages: &str) -> Citation {
    let mut c = cite(tag);
    c.items[0].pages = pages.into();
    c
}

/// Check the bibliography entry and the citation of every sample in `style`.
fn check(style: StyleId, expected: &[(&str, &str, &str)]) {
    let sources = samples();
    let cited: Vec<String> = TAGS.iter().map(|t| (*t).to_owned()).collect();
    let f = Formatter::new(style, &sources, &cited);
    assert_eq!(expected.len(), TAGS.len());
    for (tag, entry, citation) in expected {
        let e = f.entry(tag, None).expect(tag);
        assert_eq!(Rich(e.spans).marked(), *entry, "{style:?} entry of {tag}");
        assert_eq!(
            f.citation(&cite(tag)),
            *citation,
            "{style:?} citation of {tag}"
        );
    }
}

#[test]
fn apa_7() {
    check(
        StyleId::Apa,
        &[
            // 10.2 book: Author, A. A., & Author, B. B. (year). *Title* (2nd ed.). Publisher.
            // (9.8 "&" before the last author; 9.29 no publisher place.) 8.17: two authors
            // are joined by "&" in a parenthetical citation.
            (
                "book",
                "Smith, J. K., & Doe, J. (2020). *Research methods* (2nd ed.). Blue Press.",
                "(Smith & Doe, 2020)",
            ),
            // 10.3 chapter in an edited book: chapter title plain; "In E. Editor (Ed.),
            // *Book* (pp. 45–67). Publisher."; en dash in the page range (6.6).
            (
                "chap",
                "Lee, A. (2019). Sampling. In M. Brown (Ed.), *Handbook of statistics* (pp. 45–67). Blue Press.",
                "(Lee, 2019)",
            ),
            // 10.1 journal article: journal title and volume italic, issue in parentheses
            // not italic, page range, DOI as https://doi.org link without final stop (9.35).
            // 8.17: three or more authors → first author "et al.".
            (
                "art",
                "Nguyen, M. T., Smith, J. K., & Tran, L. (2021). Deep learning for text. *Journal of Data*, *12*(3), 100–120. https://doi.org/10.1000/jd.2021.5",
                "(Nguyen et al., 2021)",
            ),
            // 10.5 conference paper published in proceedings: like a chapter.
            (
                "conf",
                "Garcia, L. (2018). Fast parsing. In *Proceedings of the International Conference on Parsing* (pp. 10–15). ACM.",
                "(Garcia, 2018)",
            ),
            // 10.4 report by a group author: the group's name in full, a stop after it.
            (
                "rep",
                "World Health Organization. (2022). *Global report*. WHO Press.",
                "(World Health Organization, 2022)",
            ),
            // 10.16 webpage: fuller date "(2023, May 4)", italic title, site name, URL; no
            // retrieval date for a page that is not designed to change (9.16).
            (
                "web",
                "Blue Docs Team. (2023, May 4). *Getting started*. Blue Docs. https://example.org/start",
                "(Blue Docs Team, 2023)",
            ),
            // 10.6 thesis: "*Title* [Type of thesis, Institution]."
            (
                "thes",
                "Pham, H. (2017). *Graph algorithms* [PhD thesis, Hanoi University].",
                "(Pham, 2017)",
            ),
            // Law text (adaptation): issuing body as group author, number after the title.
            (
                "law",
                "Chính phủ. (2023). *Nghị định quy định về bảo vệ dữ liệu cá nhân* (13/2023/NĐ-CP).",
                "(Chính phủ, 2023)",
            ),
        ],
    );
}

#[test]
fn mla_9() {
    check(
        StyleId::Mla,
        &[
            // 5.7 two authors: first inverted, "and", second in normal order. Book: *Title*.
            // Version, Publisher, Year. 6.5: citation "(Smith and Doe)" (no year).
            (
                "book",
                "Smith, John K., and Jane Doe. *Research methods*. 2nd ed., Blue Press, 2020.",
                "(Smith and Doe)",
            ),
            // 5.31 work in a collection: "Title." *Container*, edited by …, Publisher, Year,
            // pp. 45–67.
            (
                "chap",
                "Lee, Anna. “Sampling.” *Handbook of statistics*, edited by Mark Brown, Blue Press, 2019, pp. 45–67.",
                "(Lee)",
            ),
            // 5.8 three or more authors: first author "et al."; 5.51 "vol. 12, no. 3";
            // 5.93 DOI as https://doi.org link.
            (
                "art",
                "Nguyen, Minh T., et al. “Deep learning for text.” *Journal of Data*, vol. 12, no. 3, 2021, pp. 100–120, https://doi.org/10.1000/jd.2021.5.",
                "(Nguyen et al.)",
            ),
            (
                "conf",
                "Garcia, Luis. “Fast parsing.” *Proceedings of the International Conference on Parsing*, ACM, 2018, pp. 10–15.",
                "(Garcia)",
            ),
            // 5.17 organisation as author (it differs from the publisher, so it is listed).
            (
                "rep",
                "World Health Organization. *Global report*. WHO Press, 2022.",
                "(World Health Organization)",
            ),
            // Web page: "Title." *Site*, day month year (5.68–5.83, months abbreviated),
            // URL. Accessed day month year (5.111).
            (
                "web",
                "Blue Docs Team. “Getting started.” *Blue Docs*, 4 May 2023, https://example.org/start. Accessed 15 Jan. 2024.",
                "(Blue Docs Team)",
            ),
            // Dissertation: *Title*. Year. Institution, kind of thesis.
            (
                "thes",
                "Pham, Hoa. *Graph algorithms*. 2017. Hanoi University, PhD thesis.",
                "(Pham)",
            ),
            // Law text (adaptation): government body as author, number and date.
            (
                "law",
                "Chính phủ. *Nghị định quy định về bảo vệ dữ liệu cá nhân*. 13/2023/NĐ-CP, 17 Apr. 2023.",
                "(Chính phủ)",
            ),
        ],
    );
}

#[test]
fn chicago_author_date() {
    check(
        StyleId::Chicago,
        &[
            // 15.9: Author. Year. *Title*. Edition. Place: Publisher. First author inverted.
            // 15.22: citation "(Smith and Doe 2020)" — no comma before the year.
            (
                "book",
                "Smith, John K., and Jane Doe. 2020. *Research methods*. 2nd ed. Hanoi: Blue Press.",
                "(Smith and Doe 2020)",
            ),
            // 15.9 chapter: "Title." In *Book*, edited by Editor, pages. Place: Publisher.
            (
                "chap",
                "Lee, Anna. 2019. “Sampling.” In *Handbook of statistics*, edited by Mark Brown, 45–67. London: Blue Press.",
                "(Lee 2019)",
            ),
            // 15.9 article: *Journal* volume (issue): pages. DOI link, then a full stop.
            // 15.29: three authors are all named in the citation, serial comma.
            (
                "art",
                "Nguyen, Minh T., John K. Smith, and Lan Tran. 2021. “Deep learning for text.” *Journal of Data* 12 (3): 100–120. https://doi.org/10.1000/jd.2021.5.",
                "(Nguyen, Smith, and Tran 2021)",
            ),
            (
                "conf",
                "Garcia, Luis. 2018. “Fast parsing.” In *Proceedings of the International Conference on Parsing*, 10–15. Paris: ACM.",
                "(Garcia 2018)",
            ),
            (
                "rep",
                "World Health Organization. 2022. *Global report*. Geneva: WHO Press.",
                "(World Health Organization 2022)",
            ),
            // 15.50 / 14.207 web page: "Title." Site. Month day, year. URL.
            (
                "web",
                "Blue Docs Team. 2023. “Getting started.” Blue Docs. May 4, 2023. https://example.org/start.",
                "(Blue Docs Team 2023)",
            ),
            // 14.215 thesis: title in quotation marks, "kind of thesis, Institution".
            (
                "thes",
                "Pham, Hoa. 2017. “Graph algorithms.” PhD thesis, Hanoi University.",
                "(Pham 2017)",
            ),
            (
                "law",
                "Chính phủ. 2023. *Nghị định quy định về bảo vệ dữ liệu cá nhân*. 13/2023/NĐ-CP.",
                "(Chính phủ 2023)",
            ),
        ],
    );
}

#[test]
fn ieee() {
    // Numbered in order of first citation (the samples are cited in list order).
    check(
        StyleId::Ieee,
        &[
            // Book: J. K. Author, *Title*, xth ed. City: Publisher, year.
            (
                "book",
                "J. K. Smith and J. Doe, *Research methods*, 2nd ed. Hanoi: Blue Press, 2020.",
                "[1]",
            ),
            // Chapter: "Title," in *Book*, E. Editor, Ed. City: Publisher, year, pp. x–y.
            (
                "chap",
                "A. Lee, “Sampling,” in *Handbook of statistics*, M. Brown, Ed. London: Blue Press, 2019, pp. 45–67.",
                "[2]",
            ),
            // Periodical: "Title," *Journal*, vol. x, no. x, pp. x–y, year, doi: ….
            (
                "art",
                "M. T. Nguyen, J. K. Smith, and L. Tran, “Deep learning for text,” *Journal of Data*, vol. 12, no. 3, pp. 100–120, 2021, doi: 10.1000/jd.2021.5.",
                "[3]",
            ),
            // Conference paper: "Title," in *Proceedings*, City, year, pp. x–y.
            (
                "conf",
                "L. Garcia, “Fast parsing,” in *Proceedings of the International Conference on Parsing*, Paris, 2018, pp. 10–15.",
                "[4]",
            ),
            // Report: Author, "Title," Company, City, year.
            (
                "rep",
                "World Health Organization, “Global report,” WHO Press, Geneva, 2022.",
                "[5]",
            ),
            // Online: Author. "Title." Site. Accessed: Mon. day, year. [Online]. Available: URL
            (
                "web",
                "Blue Docs Team. “Getting started.” Blue Docs. Accessed: Jan. 15, 2024. [Online]. Available: https://example.org/start",
                "[6]",
            ),
            // Thesis: "Title," kind of thesis, University, City, year.
            (
                "thes",
                "H. Pham, “Graph algorithms,” PhD thesis, Hanoi University, Hanoi, 2017.",
                "[7]",
            ),
            (
                "law",
                "Chính phủ, “Nghị định quy định về bảo vệ dữ liệu cá nhân,” 13/2023/NĐ-CP, 2023.",
                "[8]",
            ),
        ],
    );
}

#[test]
fn harvard() {
    check(
        StyleId::Harvard,
        &[
            // Cite Them Right, book: Author, A.B. and Author, C. (year) *Title*. 2nd edn.
            // Place: Publisher. Citation: (Smith and Doe, 2020).
            (
                "book",
                "Smith, J.K. and Doe, J. (2020) *Research methods*. 2nd edn. Hanoi: Blue Press.",
                "(Smith and Doe, 2020)",
            ),
            // Chapter: 'Title', in Editor, E. (ed.) *Book*. Place: Publisher, pp. x–y.
            (
                "chap",
                "Lee, A. (2019) ‘Sampling’, in Brown, M. (ed.) *Handbook of statistics*. London: Blue Press, pp. 45–67.",
                "(Lee, 2019)",
            ),
            // Article: 'Title', *Journal*, volume(issue), pp. x–y. Available at: DOI link.
            // Up to three authors are named in the citation.
            (
                "art",
                "Nguyen, M.T., Smith, J.K. and Tran, L. (2021) ‘Deep learning for text’, *Journal of Data*, 12(3), pp. 100–120. Available at: https://doi.org/10.1000/jd.2021.5.",
                "(Nguyen, Smith and Tran, 2021)",
            ),
            // Conference paper: 'Title', *Conference*. Place: Publisher, pp. x–y.
            (
                "conf",
                "Garcia, L. (2018) ‘Fast parsing’, *Proceedings of the International Conference on Parsing*. Paris: ACM, pp. 10–15.",
                "(Garcia, 2018)",
            ),
            (
                "rep",
                "World Health Organization (2022) *Global report*. Geneva: WHO Press.",
                "(World Health Organization, 2022)",
            ),
            // Web page: *Title*. Available at: URL (Accessed: day month year).
            (
                "web",
                "Blue Docs Team (2023) *Getting started*. Available at: https://example.org/start (Accessed: 15 January 2024).",
                "(Blue Docs Team, 2023)",
            ),
            // Thesis: *Title*. Level. Awarding body.
            (
                "thes",
                "Pham, H. (2017) *Graph algorithms*. PhD thesis. Hanoi University.",
                "(Pham, 2017)",
            ),
            (
                "law",
                "Chính phủ (2023) *Nghị định quy định về bảo vệ dữ liệu cá nhân* (13/2023/NĐ-CP).",
                "(Chính phủ, 2023)",
            ),
        ],
    );
}

#[test]
fn iso_690_name_and_date() {
    check(
        StyleId::Iso690,
        &[
            // A.2: the year follows the creators. 5.4: family name (capitals), given names.
            // Book: *Title*. Edition. Place: Publisher.
            (
                "book",
                "SMITH, John K. and DOE, Jane, 2020. *Research methods*. 2nd ed. Hanoi: Blue Press.",
                "(Smith and Doe 2020)",
            ),
            // 15.2 contribution: Title. In: EDITOR (ed.), *Host*. Place: Publisher, pages.
            (
                "chap",
                "LEE, Anna, 2019. Sampling. In: BROWN, Mark (ed.), *Handbook of statistics*. London: Blue Press, pp. 45–67.",
                "(Lee 2019)",
            ),
            // 15.3 article: Title. *Journal*. Vol. x, no. x, pp. x–y. DOI.
            (
                "art",
                "NGUYEN, Minh T., SMITH, John K. and TRAN, Lan, 2021. Deep learning for text. *Journal of Data*. Vol. 12, no. 3, pp. 100–120. DOI 10.1000/jd.2021.5.",
                "(Nguyen et al. 2021)",
            ),
            (
                "conf",
                "GARCIA, Luis, 2018. Fast parsing. In: *Proceedings of the International Conference on Parsing*. Paris: ACM, pp. 10–15.",
                "(Garcia 2018)",
            ),
            (
                "rep",
                "World Health Organization, 2022. *Global report*. Geneva: WHO Press.",
                "(World Health Organization 2022)",
            ),
            // 15.7 online: *Title* [online]. Site. [Accessed date]. Available from: URL
            (
                "web",
                "Blue Docs Team, 2023. *Getting started* [online]. Blue Docs. [Accessed 15 January 2024]. Available from: https://example.org/start",
                "(Blue Docs Team 2023)",
            ),
            (
                "thes",
                "PHAM, Hoa, 2017. *Graph algorithms*. PhD thesis. Hanoi: Hanoi University.",
                "(Pham 2017)",
            ),
            (
                "law",
                "Chính phủ, 2023. *Nghị định quy định về bảo vệ dữ liệu cá nhân*. 13/2023/NĐ-CP.",
                "(Chính phủ 2023)",
            ),
        ],
    );
}

#[test]
fn iso_690_numeric() {
    check(
        StyleId::Iso690Numeric,
        &[
            // A.3: numbered in order of first citation; the year stays with the imprint.
            (
                "book",
                "SMITH, John K. and DOE, Jane. *Research methods*. 2nd ed. Hanoi: Blue Press, 2020.",
                "[1]",
            ),
            (
                "chap",
                "LEE, Anna. Sampling. In: BROWN, Mark (ed.), *Handbook of statistics*. London: Blue Press, 2019, pp. 45–67.",
                "[2]",
            ),
            (
                "art",
                "NGUYEN, Minh T., SMITH, John K. and TRAN, Lan. Deep learning for text. *Journal of Data*. 2021, vol. 12, no. 3, pp. 100–120. DOI 10.1000/jd.2021.5.",
                "[3]",
            ),
            (
                "conf",
                "GARCIA, Luis. Fast parsing. In: *Proceedings of the International Conference on Parsing*. Paris: ACM, 2018, pp. 10–15.",
                "[4]",
            ),
            (
                "rep",
                "World Health Organization. *Global report*. Geneva: WHO Press, 2022.",
                "[5]",
            ),
            (
                "web",
                "Blue Docs Team. *Getting started* [online]. Blue Docs, 2023. [Accessed 15 January 2024]. Available from: https://example.org/start",
                "[6]",
            ),
            (
                "thes",
                "PHAM, Hoa. *Graph algorithms*. PhD thesis. Hanoi: Hanoi University, 2017.",
                "[7]",
            ),
            (
                "law",
                "Chính phủ. *Nghị định quy định về bảo vệ dữ liệu cá nhân*. 13/2023/NĐ-CP, 2023.",
                "[8]",
            ),
        ],
    );
}

#[test]
fn vietnamese_thesis() {
    // Order: the Vietnamese-language source (the decree) first, then the foreign ones by
    // family name / body: Blue Docs Team, Garcia, Lee, Nguyen, Pham, Smith, World Health
    // Organization.
    check(
        StyleId::VnThesis,
        &[
            // Sách: Tác giả (năm), *Tên sách*, Nhà xuất bản, Nơi xuất bản. Foreign authors:
            // family name and initials, no comma inside a name.
            (
                "book",
                "Smith J.K., Doe J. (2020), *Research methods*, Blue Press, Hanoi.",
                "[7]",
            ),
            // Phần của sách: "Tên bài", *Tên sách*, NXB, nơi XB, trang ("pp." for a foreign
            // source; the range keeps its hyphen).
            (
                "chap",
                "Lee A. (2019), “Sampling”, *Handbook of statistics*, Blue Press, London, pp. 45-67.",
                "[4]",
            ),
            // Bài báo: "Tên bài", *Tên tạp chí*, tập(số), các số trang.
            (
                "art",
                "Nguyen M.T., Smith J.K., Tran L. (2021), “Deep learning for text”, *Journal of Data*, 12(3), pp. 100-120, https://doi.org/10.1000/jd.2021.5.",
                "[5]",
            ),
            (
                "conf",
                "Garcia L. (2018), “Fast parsing”, *Proceedings of the International Conference on Parsing*, ACM, Paris, pp. 10-15.",
                "[3]",
            ),
            // Tài liệu không có tên tác giả: cơ quan ban hành.
            (
                "rep",
                "World Health Organization (2022), *Global report*, WHO Press, Geneva.",
                "[8]",
            ),
            (
                "web",
                "Blue Docs Team (2023), “Getting started”, *Blue Docs*, https://example.org/start, accessed 15/01/2024.",
                "[2]",
            ),
            // Luận án: *Tên luận án*, loại luận án, cơ sở đào tạo, nơi bảo vệ.
            (
                "thes",
                "Pham H. (2017), *Graph algorithms*, PhD thesis, Hanoi University, Hanoi.",
                "[6]",
            ),
            // Văn bản pháp luật: cơ quan ban hành (năm), *tên văn bản*, số hiệu, ngày.
            (
                "law",
                "Chính phủ (2023), *Nghị định quy định về bảo vệ dữ liệu cá nhân*, số 13/2023/NĐ-CP, ngày 17/04/2023.",
                "[1]",
            ),
        ],
    );
}

/// The examples of the Ministry's guidance: Vietnamese names in their usual order, sorted by
/// given name; Vietnamese sources before foreign ones; "tr." for Vietnamese page numbers.
#[test]
fn vietnamese_thesis_order_and_names() {
    let vi = |mut s: Source| {
        s.set("LCID", "1066");
        s
    };
    let sources = vec![
        source(
            "And85",
            "JournalArticle",
            vec![people("Author", &[("Anderson", "J. E.")])],
            &[
                ("Title", "The Relative Inefficiency of Quota"),
                ("JournalName", "American Economic Review"),
                ("Year", "1985"),
                ("Volume", "75"),
                ("Issue", "1"),
                ("Pages", "178-90"),
            ],
        ),
        vi(source(
            "Qua92",
            "JournalArticle",
            vec![people("Author", &[("Quách", "Ngọc Ân")])],
            &[
                ("Title", "Nhìn lại hai năm phát triển lúa lai"),
                ("JournalName", "Di truyền học ứng dụng"),
                ("Year", "1992"),
                ("Volume", "98"),
                ("Issue", "1"),
                ("Pages", "10-16"),
            ],
        )),
        vi(source(
            "Bo96",
            "Report",
            vec![body("Author", "Bộ Nông nghiệp & PTNT")],
            &[
                (
                    "Title",
                    "Báo cáo tổng kết 5 năm (1992-1996) phát triển lúa lai",
                ),
                ("Year", "1996"),
                ("City", "Hà Nội"),
            ],
        )),
        vi(source(
            "Ngu15",
            "Book",
            vec![people(
                "Author",
                &[("Nguyễn", "Văn An"), ("Trần", "Thị Bình")],
            )],
            &[
                ("Title", "Giáo trình thống kê"),
                ("Year", "2015"),
                ("Publisher", "Nhà xuất bản Giáo dục"),
                ("City", "Hà Nội"),
            ],
        )),
        source(
            "Bor97",
            "JournalArticle",
            vec![people(
                "Author",
                &[("Borkakati", "R. P."), ("Virmani", "S. S.")],
            )],
            &[
                (
                    "Title",
                    "Genetics of thermosensitive genic male sterility in Rice",
                ),
                ("JournalName", "Euphytica"),
                ("Year", "1997"),
                ("Volume", "88"),
                ("Pages", "1-7"),
            ],
        ),
    ];
    let f = Formatter::new(StyleId::VnThesis, &sources, &[]);
    let list: Vec<String> = f
        .bibliography(None)
        .iter()
        .map(super::Entry::marked)
        .collect();
    assert_eq!(
        list,
        [
            // Vietnamese sources by given name: An (Nguyễn Văn An) < Ân (Quách Ngọc Ân) <
            // "Bộ …" (no author: the issuing body's name).
            "[1] Nguyễn Văn An, Trần Thị Bình (2015), *Giáo trình thống kê*, Nhà xuất bản Giáo dục, Hà Nội.",
            "[2] Quách Ngọc Ân (1992), “Nhìn lại hai năm phát triển lúa lai”, *Di truyền học ứng dụng*, 98(1), tr. 10-16.",
            "[3] Bộ Nông nghiệp & PTNT (1996), *Báo cáo tổng kết 5 năm (1992-1996) phát triển lúa lai*, Hà Nội.",
            // Then the foreign sources by family name.
            "[4] Anderson J.E. (1985), “The Relative Inefficiency of Quota”, *American Economic Review*, 75(1), pp. 178-90.",
            "[5] Borkakati R.P., Virmani S.S. (1997), “Genetics of thermosensitive genic male sterility in Rice”, *Euphytica*, 88, pp. 1-7.",
        ]
    );
    // "[15, tr. 314-315]": number and pages; several sources: "[19], [25]".
    assert_eq!(f.citation(&cite_pages("Qua92", "10-16")), "[2, tr. 10-16]");
    assert_eq!(f.citation(&cite_pages("And85", "180")), "[4, p. 180]");
    let mut two = cite("Ngu15");
    two.items.push(CiteItem {
        tag: "Bor97".into(),
        ..CiteItem::default()
    });
    assert_eq!(f.citation(&two), "[1], [5]");
}

/// Page locators in a citation, per style.
#[test]
fn citation_pages() {
    let sources = samples();
    let cited = vec!["book".to_owned()];
    let page = |style: StyleId, pages: &str| {
        Formatter::new(style, &sources, &cited).citation(&cite_pages("book", pages))
    };
    // APA 8.13: "p." for one page, "pp." for a range, en dash.
    assert_eq!(page(StyleId::Apa, "5"), "(Smith & Doe, 2020, p. 5)");
    assert_eq!(page(StyleId::Apa, "5-7"), "(Smith & Doe, 2020, pp. 5–7)");
    // MLA 6.2: author and page number, nothing between them.
    assert_eq!(page(StyleId::Mla, "5"), "(Smith and Doe 5)");
    // Chicago 15.23: a comma before the page, no "p.".
    assert_eq!(page(StyleId::Chicago, "5-7"), "(Smith and Doe 2020, 5–7)");
    // Cite Them Right: (Author, year, p. x).
    assert_eq!(page(StyleId::Harvard, "5"), "(Smith and Doe, 2020, p. 5)");
    // ISO 690 A.2: (Name year, p. x).
    assert_eq!(page(StyleId::Iso690, "5"), "(Smith and Doe 2020, p. 5)");
    // IEEE: [1, p. 5], [1, pp. 5–7].
    assert_eq!(page(StyleId::Ieee, "5"), "[1, p. 5]");
    assert_eq!(page(StyleId::Ieee, "5-7"), "[1, pp. 5–7]");
    assert_eq!(page(StyleId::Iso690Numeric, "5"), "[1, p. 5]");
}

/// §17.16.5.8: `\n` `\y` `\t` leave author / year / title out, `\f` / `\s` add text, `\m`
/// adds a source, `\v` a volume, `\l` sets the language of the added words.
#[test]
fn citation_switches() {
    let sources = samples();
    let f = Formatter::new(StyleId::Apa, &sources, &[]);
    let c = Citation::parse(
        r#"CITATION book \l 1033 \p "12-15" \n \m art \f "see " \s " for more" \m thes \y"#,
    )
    .unwrap();
    assert_eq!(c.items.len(), 3);
    assert_eq!(c.lcid.as_deref(), Some("1033"));
    assert!(c.items[0].no_author && c.items[2].no_year);
    assert_eq!(
        f.citation(&c),
        "(2020, pp. 12–15; see Nguyen et al., 2021 for more; Pham)"
    );
    // The instruction round-trips.
    assert_eq!(Citation::parse(&c.instr()).unwrap(), c);
    assert_eq!(
        c.instr(),
        r#"CITATION book \l 1033 \p "12-15" \n \m art \f "see " \s " for more" \m thes \y"#
    );
    // Volume and page: Chicago "2:5", MLA "2: 5", APA "Vol. 2, p. 5".
    let mut v = cite_pages("book", "5");
    v.items[0].volume = "2".into();
    assert_eq!(f.citation(&v), "(Smith & Doe, 2020, Vol. 2, p. 5)");
    let ch = Formatter::new(StyleId::Chicago, &sources, &[]);
    assert_eq!(ch.citation(&v), "(Smith and Doe 2020, 2:5)");
    let mla = Formatter::new(StyleId::Mla, &sources, &[]);
    assert_eq!(mla.citation(&v), "(Smith and Doe 2: 5)");
    // Vietnamese words with `\l 1066`: "và", "và cộng sự", "tr.".
    let mut vi = cite_pages("art", "5");
    vi.lcid = Some("1066".into());
    assert_eq!(f.citation(&vi), "(Nguyen và cộng sự, 2021, tr. 5)");
    let mut vi2 = cite("book");
    vi2.lcid = Some("1066".into());
    assert_eq!(f.citation(&vi2), "(Smith và Doe, 2020)");
    // Another field is not a citation.
    assert!(Citation::parse("BIBLIOGRAPHY").is_none());
    assert!(Citation::parse("CITATION").is_none());
}

/// A placeholder (and a tag without source) shows its tag and is left out of the list.
#[test]
fn placeholders() {
    let mut sources = samples();
    sources.push(source("Placeholder1", "Book", vec![], &[]));
    assert!(sources.last().unwrap().is_placeholder());
    for style in StyleId::ALL {
        let f = Formatter::new(style, &sources, &["Placeholder1".to_owned()]);
        let expect = if style.numeric() {
            "[Placeholder1]"
        } else {
            "(Placeholder1)"
        };
        assert_eq!(f.citation(&cite("Placeholder1")), expect, "{style:?}");
        assert!(f.citation(&cite("Missing")).contains("Missing"));
        assert_eq!(f.bibliography(None).len(), 8, "{style:?}");
        assert!(f.entry("Placeholder1", None).is_none());
    }
}

/// Alphabetical lists, the letter after the year for the same author and year, numbering
/// by first citation.
#[test]
fn order_and_disambiguation() {
    let a = |tag: &str, title: &str| {
        source(
            tag,
            "Book",
            vec![people("Author", &[("Smith", "John")])],
            &[("Title", title), ("Year", "2020"), ("Publisher", "P")],
        )
    };
    let sources = vec![
        a("s2", "Zebra"),
        a("s1", "Apple"),
        source(
            "d",
            "Book",
            vec![people("Author", &[("Doe", "Jane")])],
            &[("Title", "Middle"), ("Year", "2001")],
        ),
        // No author: sorted by title without its article (APA 9.49).
        source(
            "t",
            "Book",
            vec![],
            &[("Title", "The quiet book"), ("Year", "1999")],
        ),
    ];
    let f = Formatter::new(StyleId::Apa, &sources, &[]);
    let list: Vec<String> = f
        .bibliography(None)
        .iter()
        .map(super::Entry::marked)
        .collect();
    assert_eq!(
        list,
        [
            "Doe, J. (2001). *Middle*.",
            // 9.12: the title in the author position.
            "*The quiet book*. (1999).",
            // APA 9.47: same author and year → a, b in title order.
            "Smith, J. (2020a). *Apple*. P.",
            "Smith, J. (2020b). *Zebra*. P.",
        ]
    );
    assert_eq!(f.citation(&cite("s2")), "(Smith, 2020b)");
    assert_eq!(f.citation(&cite("t")), "(The quiet book, 1999)");
    // Harvard: title first in italics when there is no author.
    let h = Formatter::new(StyleId::Harvard, &sources, &[]);
    assert_eq!(
        h.entry("t", None).unwrap().marked(),
        "*The quiet book* (1999)."
    );
    // IEEE numbers by first citation; uncited sources follow in list order.
    let cited = vec!["d".to_owned(), "s1".to_owned(), "d".to_owned()];
    let i = Formatter::new(StyleId::Ieee, &sources, &cited);
    let tags: Vec<String> = i.bibliography(None).into_iter().map(|e| e.tag).collect();
    assert_eq!(tags, ["d", "s1", "s2", "t"]);
    assert_eq!(i.citation(&cite("s1")), "[2]");
    assert_eq!(
        i.bibliography(None)[0].marked(),
        "[1] J. Doe, *Middle*, 2001."
    );
}

/// Author lists at their limits.
#[test]
fn long_author_lists() {
    let many = |n: usize| -> Vec<(String, String)> {
        (0..n)
            .map(|i| (format!("Author{:02}", i + 1), "A.".to_owned()))
            .collect()
    };
    let src = |n: usize| {
        let names = many(n);
        let refs: Vec<(&str, &str)> = names
            .iter()
            .map(|(l, f)| (l.as_str(), f.as_str()))
            .collect();
        vec![source(
            "x",
            "Book",
            vec![people("Author", &refs)],
            &[("Title", "T"), ("Year", "2000")],
        )]
    };
    // APA 9.8: 21 or more authors → the first 19, an ellipsis, the last.
    let s = src(21);
    let e = Formatter::new(StyleId::Apa, &s, &[])
        .entry("x", None)
        .unwrap()
        .marked();
    assert!(e.starts_with("Author01, A., Author02, A., "), "{e}");
    assert!(
        e.contains("Author19, A., . . . Author21, A. (2000)."),
        "{e}"
    );
    assert!(!e.contains("Author20"), "{e}");
    // IEEE: more than six authors → the first and "et al.".
    let s = src(7);
    let e = Formatter::new(StyleId::Ieee, &s, &[])
        .entry("x", None)
        .unwrap();
    assert_eq!(Rich(e.spans).marked(), "A. Author01 et al., *T*, 2000.");
    // Chicago 14.76: more than ten → the first seven and "et al.".
    let s = src(11);
    let e = Formatter::new(StyleId::Chicago, &s, &[])
        .entry("x", None)
        .unwrap()
        .marked();
    assert!(e.contains("A. Author07, et al. 2000."), "{e}");
    assert!(!e.contains("Author08"), "{e}");
    // Chicago 15.29: four or more authors in the citation → "et al.".
    let s = src(4);
    assert_eq!(
        Formatter::new(StyleId::Chicago, &s, &[]).citation(&cite("x")),
        "(Author01 et al. 2000)"
    );
}

/// MLA 5.20: an organisation that is author and publisher is listed as publisher only.
#[test]
fn mla_organisation_as_publisher() {
    let s = vec![source(
        "o",
        "Report",
        vec![body("Author", "Blue Institute")],
        &[
            ("Title", "Annual review"),
            ("Year", "2024"),
            ("Publisher", "Blue Institute"),
        ],
    )];
    let f = Formatter::new(StyleId::Mla, &s, &[]);
    assert_eq!(
        f.entry("o", None).unwrap().marked(),
        "*Annual review*. Blue Institute, 2024."
    );
}

#[test]
fn style_names() {
    for s in StyleId::ALL {
        assert_eq!(StyleId::from_name(s.id()), s);
    }
    // Names other programs write.
    assert_eq!(
        StyleId::from_name("ISO 690 - Numerical Reference"),
        StyleId::Iso690Numeric
    );
    assert_eq!(
        StyleId::from_name("ISO 690 - First Element and Date"),
        StyleId::Iso690
    );
    assert_eq!(StyleId::from_name("Harvard - Anglia"), StyleId::Harvard);
    assert_eq!(StyleId::from_name("IEEE"), StyleId::Ieee);
    assert_eq!(StyleId::from_name(""), StyleId::Apa);
}
