//! Bundled sample corpus for the Ontology Studio one-command demo (`studio --demo`).
//!
//! Mirrors a realistic SME document set so a prospect can see the whole wedge
//! without gathering files: a shift report, a purchase-order email, a
//! machine-generated PDF invoice, and an off-grid cybersecurity incident (to
//! showcase gap discovery). Writing is offline (no downloads) and idempotent —
//! existing files are never clobbered.

/// A minimal but structurally valid PDF (xref + a text content stream) so the
/// bundled demo can exercise the optional `pdf` extraction line for real.
/// Contains the strings "ACME INDUSTRIES S.P.A.", "Invoice No. 2026-088",
/// "Date: 2026-05-10" so extraction is verifiable.
const MINIMAL_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>
endobj
4 0 obj
<< /Length 119 >>
stream
BT /F1 12 Tf 72 720 Td (ACME INDUSTRIES S.P.A.) Tj 0 -18 Td (Invoice No. 2026-088) Tj 0 -18 Td (Date: 2026-05-10) Tj ET
endstream
endobj
5 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000241 00000 n 
0000000411 00000 n 
trailer
<< /Size 6 /Root 1 0 R >>
startxref
481
%%EOF
";

/// The (text) sample documents, keyed by filename.
fn demo_samples() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "shift_report.md",
            "Shift report — Line 3, night shift.\n\
             Production ran at 92% OEE. Compressor C-2 tripped on high temperature at 03:10; \
             operator reset and resumed within 8 minutes. Maintenance notified for bearing inspection.\n\
             Two quality holds: lot #4471 (dimensional), lot #223 (surface). Both released after rework.\n\
             SAP logged 1,840 units good, 22 scrap. Downtime 11 min total.",
        ),
        (
            "po_confirmed.eml",
            "From: procurement@acme-industries.com\n\
             To: ops@client.example\n\
             Subject: PO 4471 confirmed\n\
             Date: Thu, 08 May 2026 09:14:00 +0200\n\
             \n\
             Confirmed purchase order 4471 for 500 units of bracket assembly.\n\
             Vendor: ACME INDUSTRIES S.P.A. Delivery week 22. Quote ref 2026-088.\n",
        ),
        (
            "invoice_acme.pdf",
            "", // body ignored; MINIMAL_PDF written as bytes
        ),
        (
            "security_incident.md",
            "Security incident report. At 02:14 an unauthorized login was detected on the OT network \
             via a compromised VPN credential. The SOC escalated to IR; the account was disabled and \
             the session terminated. Forensic capture saved to the SIEM. 223 successful logins preceded \
             the breach; 185 remote sessions reviewed. Recommend MFA on all access points and network \
             segmentation for the 4471 production VLAN.",
        ),
    ]
}

/// Seed `dir` with the bundled sample corpus (creating `dir` if missing).
/// Idempotent: files that already exist are left untouched, so re-running never
/// clobbers a prospect's data.
pub fn seed_demo_corpus(dir: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, body) in demo_samples() {
        let p = dir.join(name);
        if p.exists() {
            continue;
        }
        if name.ends_with(".pdf") {
            std::fs::write(&p, MINIMAL_PDF)?;
        } else {
            std::fs::write(&p, body)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_is_idempotent_and_offline() {
        let tmp = std::env::temp_dir().join("physis_demo_seed_test");
        let _ = std::fs::remove_dir_all(&tmp);
        seed_demo_corpus(&tmp).unwrap();
        // re-run must not error and must not overwrite an edited file
        let report = tmp.join("shift_report.md");
        std::fs::write(&report, "TAMPERED").unwrap();
        seed_demo_corpus(&tmp).unwrap();
        assert_eq!(std::fs::read_to_string(&report).unwrap(), "TAMPERED");
        // pdf is a real, non-empty file
        let pdf = tmp.join("invoice_acme.pdf");
        assert!(pdf.exists() && std::fs::metadata(&pdf).unwrap().len() > 50);
        // pdf contains the verifiable invoice strings
        let bytes = std::fs::read(&pdf).unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("ACME INDUSTRIES S.P.A."));
        assert!(s.contains("Invoice No. 2026-088"));
        assert!(s.contains("Date: 2026-05-10"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
