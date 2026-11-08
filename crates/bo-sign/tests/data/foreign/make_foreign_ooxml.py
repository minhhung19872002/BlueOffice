"""Builds `foreign-signed.docx`: an OOXML package signature written WITHOUT BlueOffice code, to test
that `bo_sign::ooxml::verify_ooxml` accepts other producers' serialisations.

Independent pieces: lxml (libxml2) for Canonical XML / Exclusive C14N, OpenSSL for the key, the
self-signed certificate and the RSA-SHA256 signature, and the relationship transform written here from
ECMA-376 Part 2 §13.2.4.24. Deliberate differences from BlueOffice's own output: `ds:` prefixes,
indentation inside SignedInfo, Exclusive C14N for SignedInfo, SHA-1 XAdES certificate digest, attribute
order, and `word/styles.xml` left out of the Manifest (a partial signature).

Usage (from the repository root):
    python crates/bo-sign/tests/data/foreign/make_foreign_ooxml.py
Writes foreign-signed.docx and foreign-cert.der next to this script.
"""
import base64
import hashlib
import io
import os
import subprocess
import tempfile
import zipfile

from lxml import etree

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "..", "..", "..", "..", "..", "tests", "corpus", "doc", "phase1-sample.docx")
DS = "http://www.w3.org/2000/09/xmldsig#"
MDSSI = "http://schemas.openxmlformats.org/package/2006/digital-signature"
XD = "http://uri.etsi.org/01903/v1.3.2#"
RELS = "http://schemas.openxmlformats.org/package/2006/relationships"
REL_TRANSFORM = "http://schemas.openxmlformats.org/package/2006/RelationshipTransform"
C14N = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315"
EXC = "http://www.w3.org/2001/10/xml-exc-c14n#"
SHA256 = "http://www.w3.org/2001/04/xmlenc#sha256"
SHA1 = "http://www.w3.org/2000/09/xmldsig#sha1"


def b64(b):
    return base64.b64encode(b).decode()


def c14n(el, exclusive=False):
    return etree.tostring(el, method="c14n", exclusive=exclusive, with_comments=False)


def rel_transform(rels_xml, ids):
    """§13.2.4.24: selected Relationship elements sorted by Id, TargetMode made explicit, then C14N."""
    root = etree.fromstring(rels_xml)
    out = etree.Element("{%s}Relationships" % RELS, nsmap={None: RELS})
    rels = [r for r in root if r.get("Id") in ids]
    for r in sorted(rels, key=lambda r: r.get("Id")):
        e = etree.SubElement(out, "{%s}Relationship" % RELS)
        for k in ("Id", "Type", "Target"):
            e.set(k, r.get(k))
        e.set("TargetMode", r.get("TargetMode") or "Internal")
    return c14n(out)


def main():
    tmp = tempfile.mkdtemp()
    key, cert_pem = os.path.join(tmp, "k.pem"), os.path.join(tmp, "c.pem")
    subprocess.run(
        ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-sha256", "-days", "3650",
         "-subj", "/CN=Foreign Signer/O=Other Vendor/C=VN", "-keyout", key, "-out", cert_pem,
         "-addext", "keyUsage=critical,digitalSignature,nonRepudiation,keyCertSign"],
        check=True, capture_output=True)
    cert_der = subprocess.run(["openssl", "x509", "-in", cert_pem, "-outform", "DER"],
                              check=True, capture_output=True).stdout
    with open(os.path.join(HERE, "foreign-cert.der"), "wb") as f:
        f.write(cert_der)

    zin = zipfile.ZipFile(SRC)
    parts = {n: zin.read(n) for n in zin.namelist()}
    ct = etree.fromstring(parts["[Content_Types].xml"])
    ctns = ct.nsmap[None]

    def content_type(name):
        for o in ct.findall("{%s}Override" % ctns):
            if o.get("PartName").lower() == "/" + name.lower():
                return o.get("ContentType")
        ext = name.rsplit(".", 1)[-1].lower()
        for d in ct.findall("{%s}Default" % ctns):
            if d.get("Extension").lower() == ext:
                return d.get("ContentType")
        return "application/octet-stream"

    # Manifest (word/styles.xml deliberately not signed).
    refs = []
    for name in sorted(parts):
        if name in ("[Content_Types].xml", "word/styles.xml"):
            continue
        uri = "/%s?ContentType=%s" % (name, content_type(name))
        if name.endswith(".rels"):
            ids = [r.get("Id") for r in etree.fromstring(parts[name])]
            digest = hashlib.sha256(rel_transform(parts[name], ids)).digest()
            tr = ('<Transforms><Transform Algorithm="%s">%s</Transform><Transform Algorithm="%s"/></Transforms>'
                  % (REL_TRANSFORM,
                     "".join('<mdssi:RelationshipReference xmlns:mdssi="%s" SourceId="%s"/>' % (MDSSI, i) for i in ids),
                     C14N))
        else:
            digest = hashlib.sha256(parts[name]).digest()
            tr = ""
        refs.append('<Reference URI="%s">%s<DigestMethod Algorithm="%s"/><DigestValue>%s</DigestValue></Reference>'
                    % (uri, tr, SHA256, b64(digest)))

    cert_sha1 = b64(hashlib.sha1(cert_der).digest())
    sig_xml = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<ds:Signature xmlns:ds="%(DS)s" Id="sig-foreign">\n'
        '  <ds:SignedInfo>\n'
        '    <ds:CanonicalizationMethod Algorithm="%(EXC)s"/>\n'
        '    <ds:SignatureMethod Algorithm="http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"/>\n'
        '    <ds:Reference URI="#pkg" Type="http://www.w3.org/2000/09/xmldsig#Object">\n'
        '      <ds:DigestMethod Algorithm="%(SHA256)s"/><ds:DigestValue>@PKG@</ds:DigestValue>\n'
        '    </ds:Reference>\n'
        '    <ds:Reference Type="http://uri.etsi.org/01903#SignedProperties" URI="#props">\n'
        '      <ds:Transforms><ds:Transform Algorithm="%(C14N)s"/></ds:Transforms>\n'
        '      <ds:DigestMethod Algorithm="%(SHA256)s"/><ds:DigestValue>@PROPS@</ds:DigestValue>\n'
        '    </ds:Reference>\n'
        '  </ds:SignedInfo>\n'
        '  <ds:SignatureValue>@SIG@</ds:SignatureValue>\n'
        '  <ds:KeyInfo><ds:X509Data><ds:X509Certificate>%(CERT)s</ds:X509Certificate></ds:X509Data></ds:KeyInfo>\n'
        '  <ds:Object Id="pkg"><ds:Manifest>%(REFS)s</ds:Manifest>'
        '<ds:SignatureProperties><ds:SignatureProperty Target="#sig-foreign" Id="time">'
        '<mdssi:SignatureTime xmlns:mdssi="%(MDSSI)s"><mdssi:Format>YYYY-MM-DDThh:mm:ssTZD</mdssi:Format>'
        '<mdssi:Value>2026-11-08T12:00:00Z</mdssi:Value></mdssi:SignatureTime></ds:SignatureProperty>'
        '</ds:SignatureProperties></ds:Object>\n'
        '  <ds:Object><xd:QualifyingProperties xmlns:xd="%(XD)s" Target="#sig-foreign">'
        '<xd:SignedProperties Id="props"><xd:SignedSignatureProperties>'
        '<xd:SigningTime>2026-11-08T12:00:00Z</xd:SigningTime><xd:SigningCertificate><xd:Cert><xd:CertDigest>'
        '<ds:DigestMethod Algorithm="%(SHA1)s"/><ds:DigestValue>%(CERTSHA1)s</ds:DigestValue>'
        '</xd:CertDigest><xd:IssuerSerial><ds:X509IssuerName>CN=Foreign Signer</ds:X509IssuerName>'
        '<ds:X509SerialNumber>1</ds:X509SerialNumber></xd:IssuerSerial></xd:Cert></xd:SigningCertificate>'
        '</xd:SignedSignatureProperties></xd:SignedProperties></xd:QualifyingProperties></ds:Object>\n'
        '</ds:Signature>'
    ) % dict(DS=DS, EXC=EXC, C14N=C14N, SHA256=SHA256, SHA1=SHA1, CERT=b64(cert_der), REFS="".join(refs),
             MDSSI=MDSSI, XD=XD, CERTSHA1=cert_sha1)

    doc = etree.fromstring(sig_xml.encode())
    ns = {"ds": DS, "xd": XD}
    pkg_obj = doc.find("ds:Object[@Id='pkg']", ns)
    props = doc.find(".//xd:SignedProperties", ns)
    refs_el = doc.findall("ds:SignedInfo/ds:Reference", ns)
    # Same-document references: Canonical XML 1.0 of the referenced element (in its context).
    refs_el[0].find("ds:DigestValue", ns).text = b64(hashlib.sha256(c14n(pkg_obj)).digest())
    refs_el[1].find("ds:DigestValue", ns).text = b64(hashlib.sha256(c14n(props)).digest())
    si = doc.find("ds:SignedInfo", ns)
    signed = c14n(si, exclusive=True)
    sig = subprocess.run(["openssl", "dgst", "-sha256", "-sign", key], input=signed,
                         check=True, capture_output=True).stdout
    doc.find("ds:SignatureValue", ns).text = b64(sig)
    sig_bytes = b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' + etree.tostring(doc)

    # Package: origin part, signature part, relationships and content types.
    parts["_xmlsignatures/sig1.xml"] = sig_bytes
    parts["_xmlsignatures/origin.sigs"] = b""
    parts["_xmlsignatures/_rels/origin.sigs.rels"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="%s">'
        '<Relationship Id="rIdS1" Type="http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/signature"'
        ' Target="sig1.xml"/></Relationships>' % RELS).encode()
    root_rels = etree.fromstring(parts["_rels/.rels"])
    r = etree.SubElement(root_rels, "{%s}Relationship" % RELS)
    r.set("Id", "rIdOrigin")
    r.set("Type", "http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/origin")
    r.set("Target", "_xmlsignatures/origin.sigs")
    parts["_rels/.rels"] = etree.tostring(root_rels, xml_declaration=True, encoding="UTF-8", standalone=True)
    for ext, t in (("sigs", "application/vnd.openxmlformats-package.digital-signature-origin"),):
        d = etree.SubElement(ct, "{%s}Default" % ctns)
        d.set("Extension", ext)
        d.set("ContentType", t)
    o = etree.SubElement(ct, "{%s}Override" % ctns)
    o.set("PartName", "/_xmlsignatures/sig1.xml")
    o.set("ContentType", "application/vnd.openxmlformats-package.digital-signature-xmlsignature+xml")
    parts["[Content_Types].xml"] = etree.tostring(ct, xml_declaration=True, encoding="UTF-8", standalone=True)

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", parts.pop("[Content_Types].xml"))
        for n, data in parts.items():
            z.writestr(n, data)
    with open(os.path.join(HERE, "foreign-signed.docx"), "wb") as f:
        f.write(buf.getvalue())


if __name__ == "__main__":
    main()
