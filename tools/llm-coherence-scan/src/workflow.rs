use crate::capture::RunRecord;
use crate::drift::DriftScore;
use crate::risk::RiskScore;

pub fn render_mermaid(rec: &RunRecord, risk: &RiskScore, drift: &DriftScore) -> String {
    let mut s = String::new();
    s.push_str("flowchart LR\n");
    let prompt_id = node_id("prompt");
    s.push_str(&format!(
        "    {prompt_id}[\"prompt<br/>risk={:.2}\"]\n",
        risk.total
    ));
    for (i, f) in rec.input.rag_files.iter().enumerate() {
        let id = node_id(&format!("rag_{i}"));
        s.push_str(&format!(
            "    {id}[\"RAG: {}<br/>path: {}\"]\n",
            short(&f.body),
            f.path
        ));
        s.push_str(&format!("    {prompt_id} -- read --> {id}\n"));
    }
    let out_id = node_id("output");
    s.push_str(&format!(
        "    {out_id}[\"assistant text<br/>{}\"]\n",
        short(&rec.output.text)
    ));
    s.push_str(&format!("    {prompt_id} -- generate --> {out_id}\n"));
    for (i, f) in rec.output.edited_files.iter().enumerate() {
        let id = node_id(&format!("edit_{i}"));
        s.push_str(&format!(
            "    {id}[\"edit: {}<br/>path: {}\"]\n",
            short(&f.body),
            f.path
        ));
        s.push_str(&format!("    {out_id} -- write --> {id}\n"));
    }
    for (i, p) in rec.output.deleted_files.iter().enumerate() {
        let id = node_id(&format!("del_{i}"));
        s.push_str(&format!("    {id}[\"deleted: {p}\"]\n"));
        s.push_str(&format!("    {out_id} -- delete --> {id}\n"));
    }
    if let Some(d) = drift.total {
        s.push_str(&format!("\n    %% drift={d:.2}\n"));
    }
    s
}

fn node_id(s: &str) -> String {
    let mut out = String::from("n_");
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

fn short(s: &str) -> String {
    let one_line: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.len() > 60 {
        format!("{}…", &one_line[..60])
    } else {
        one_line
    }
}
