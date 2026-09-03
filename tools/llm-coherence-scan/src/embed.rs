use sha2::{Digest, Sha256};

pub const EMBED_DIM: usize = 256;

pub fn hash_embed(text: &str) -> [f32; EMBED_DIM] {
    let mut out = [0.0f32; EMBED_DIM];
    for token in tokenize(text) {
        let h = Sha256::digest(token.as_bytes());
        for chunk in h.chunks(4) {
            if chunk.len() < 4 {
                break;
            }
            let idx = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize
                % EMBED_DIM;
            let sign = if chunk[0] & 1 == 0 { 1.0 } else { -1.0 };
            out[idx] += sign;
        }
    }
    let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in out.iter_mut() {
            *v /= norm;
        }
    }
    out
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    a.iter().take(n).zip(b.iter().take(n)).map(|(x, y)| x * y).sum::<f32>()
}

pub fn combine(input: &str, files: &[(String, String)]) -> String {
    let mut buf = String::with_capacity(input.len() + 256);
    buf.push_str(input);
    for (path, body) in files {
        buf.push_str("\n# ");
        buf.push_str(path);
        buf.push('\n');
        buf.push_str(body);
    }
    buf
}

fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            cur.push(c.to_ascii_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(text.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embed_is_deterministic() {
        let a = hash_embed("hello world");
        let b = hash_embed("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_embed_is_normalized() {
        let v = hash_embed("a b c d e f g h i j k l m n o p");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "norm={norm}");
    }

    #[test]
    fn cosine_identical_is_one() {
        let a = hash_embed("the quick brown fox");
        let b = hash_embed("the quick brown fox");
        assert!((cosine(&a, &b) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn cosine_unrelated_is_low() {
        let a = hash_embed("apple banana cherry");
        let b = hash_embed("quantum entanglement photon");
        let c = cosine(&a, &b);
        assert!(c < 0.5, "cosine={c}");
    }
}
