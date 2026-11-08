//! Robustness: arbitrary input never panics.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_odf::{read_odp, read_ods, read_odt, OdfPackage, MIME_ODP, MIME_ODS, MIME_ODT};
use proptest::prelude::*;

fn wrap(mime: &str, content: Vec<u8>) -> Vec<u8> {
    OdfPackage::write(
        mime,
        &[
            ("content.xml", "text/xml", content.clone()),
            ("styles.xml", "text/xml", content),
        ],
    )
    .unwrap()
}

const FRAGMENTS: &[&str] = &[
    r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0">"#,
    "<office:body>",
    "<office:text>",
    "<office:spreadsheet>",
    "<office:presentation>",
    "<office:automatic-styles>",
    r#"<style:style style:name="P1" style:family="paragraph" style:parent-style-name="P1">"#,
    r#"<style:paragraph-properties fo:line-height="1e308%" fo:margin-left="-9999999in"/>"#,
    r##"<style:text-properties fo:font-size="-3pt" fo:color="#zz"/>"##,
    "</style:style>",
    r#"<text:p text:style-name="P1">"#,
    r#"<text:s text:c="999999999999"/>"#,
    "<text:span>",
    "</text:span>",
    "</text:p>",
    r#"<table:table table:name="x">"#,
    r#"<table:table-row table:number-rows-repeated="4294967295">"#,
    r#"<table:table-cell table:number-columns-repeated="4294967295" office:value-type="float" office:value="1"/>"#,
    "</table:table-row>",
    "</table:table>",
    "<draw:page><draw:frame><draw:text-box><text:p>x</text:p></draw:text-box></draw:frame></draw:page>",
    "</office:body>",
    "</office:document-content>",
    "&amp;&#0;&#xD800;&bogus;",
];

proptest! {
    #[test]
    fn random_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let _ = read_odt(&data);
        let _ = read_ods(&data);
        let _ = read_odp(&data);
        let _ = read_odt(&wrap(MIME_ODT, data.clone()));
        let _ = read_ods(&wrap(MIME_ODS, data.clone()));
        let _ = read_odp(&wrap(MIME_ODP, data));
    }

    #[test]
    fn random_fragments_never_panic(idx in proptest::collection::vec(0..FRAGMENTS.len(), 0..40)) {
        let xml: String = idx.iter().map(|&i| FRAGMENTS[i]).collect();
        let _ = read_odt(&wrap(MIME_ODT, xml.clone().into_bytes()));
        let _ = read_ods(&wrap(MIME_ODS, xml.clone().into_bytes()));
        let _ = read_odp(&wrap(MIME_ODP, xml.into_bytes()));
    }
}
