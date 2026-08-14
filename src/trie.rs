use std::collections::HashMap;
use std::path::Path;
use std::io::Read;

const MAGIC: &[u8; 10] = b"AURA_TRIE\x00";
const VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Node {
    pub token_id: u32,
    pub child_start: i32,
    pub child_count: u32,
    pub terminal: bool,
}

#[derive(Debug, Clone)]
pub struct DynamicVectorTrie {
    pub dictionary: Vec<String>,
    pub pool: Vec<Node>,
}

impl DynamicVectorTrie {
    pub fn new() -> Self {
        let mut t = Self {
            dictionary: vec![String::new()],
            pool: Vec::new(),
        };
        t.pool.push(Node {
            token_id: 0,
            child_start: -1,
            child_count: 0,
            terminal: false,
        });
        t
    }

    pub fn token_id(&self, token: &str) -> Option<u32> {
        for (i, existing) in self.dictionary.iter().enumerate() {
            if existing == token {
                return Some(i as u32);
            }
        }
        None
    }

    pub fn token_str(&self, tid: u32) -> &str {
        let idx = tid as usize;
        if idx < self.dictionary.len() {
            &self.dictionary[idx]
        } else {
            "???"
        }
    }

    fn add_child(&mut self, parent_idx: usize, token_id: u32) -> usize {
        let cs = self.pool[parent_idx].child_start as usize;
        let cc = self.pool[parent_idx].child_count as usize;

        for i in 0..cc {
            let idx = cs + i;
            if self.pool[idx].token_id == token_id {
                return idx;
            }
        }

        let new_cs = self.pool.len() as i32;
        for i in 0..cc {
            let child = self.pool[cs + i].clone();
            self.pool.push(child);
        }
        let new_idx = self.pool.len();
        self.pool.push(Node {
            token_id,
            child_start: -1,
            child_count: 0,
            terminal: false,
        });

        self.pool[parent_idx].child_start = new_cs;
        self.pool[parent_idx].child_count = (cc + 1) as u32;
        new_idx
    }

    fn walk(&self, tokens: &[u32]) -> Option<usize> {
        let mut idx = 0usize;
        for &tid in tokens {
            let cs = self.pool[idx].child_start as usize;
            let cc = self.pool[idx].child_count as usize;
            let mut found = false;
            for i in 0..cc {
                let child = &self.pool[cs + i];
                if child.token_id == tid {
                    idx = cs + i;
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
        }
        Some(idx)
    }

    pub fn insert(&mut self, tokens: &[u32]) {
        if tokens.is_empty() {
            return;
        }
        let mut idx = 0usize;
        let len = tokens.len();
        for (i, &tid) in tokens.iter().enumerate() {
            idx = self.add_child(idx, tid);
            if i == len - 1 {
                self.pool[idx].terminal = true;
            }
        }
    }

    pub fn insert_str(&mut self, text: &str) {
        let tokens = self.tokenize_mut(text);
        self.insert(&tokens);
    }

    pub fn insert_path(&mut self, path: &str) {
        let tokens = self.tokenize_path_mut(path);
        self.insert(&tokens);
    }

    pub fn search(&self, tokens: &[u32]) -> bool {
        self.walk(tokens).is_some_and(|idx| self.pool[idx].terminal)
    }

    pub fn search_str(&self, text: &str) -> bool {
        self.search(&self.tokenize(text))
    }

    pub fn delete(&mut self, tokens: &[u32]) -> bool {
        self.walk(tokens).is_some_and(|idx| {
            if self.pool[idx].terminal {
                self.pool[idx].terminal = false;
                true
            } else {
                false
            }
        })
    }

    pub fn prefix_search(
        &self,
        tokens: &[u32],
        depth: u32,
        max_results: usize,
    ) -> Vec<Vec<String>> {
        let idx = match self.walk(tokens) {
            Some(i) => i,
            None => return vec![],
        };
        let mut results = Vec::new();
        let mut prefix: Vec<String> = Vec::new();
        self.collect_paths(idx, &mut prefix, depth, &mut results, max_results, 0);
        results
    }

    pub fn prefix_search_str(
        &self,
        words: &[&str],
        depth: u32,
        max_results: usize,
    ) -> Vec<Vec<String>> {
        let tids: Vec<u32> = words
            .iter()
            .filter_map(|w| self.token_id(w))
            .collect();
        if tids.is_empty() {
            return vec![];
        }
        self.prefix_search(&tids, depth, max_results)
    }

    pub fn prefix_search_flat(&self, words: &[&str], max_tokens: usize) -> String {
        let paths = self.prefix_search_str(words, 2, 8);
        if paths.is_empty() {
            return String::new();
        }
        let mut lines = vec!["=== WIKI CONTEXT ===".to_string()];
        let mut token_count = 0;
        for path in &paths {
            let line = path.join(" → ");
            let wt = line.split_whitespace().count();
            if token_count + wt > max_tokens && token_count > 0 {
                break;
            }
            lines.push(line);
            token_count += wt;
        }
        lines.push("=== END WIKI CONTEXT ===".to_string());
        lines.join("\n")
    }

    fn collect_paths(
        &self,
        node_idx: usize,
        prefix: &mut Vec<String>,
        depth: u32,
        results: &mut Vec<Vec<String>>,
        max_results: usize,
        cur_depth: u32,
    ) {
        if results.len() >= max_results {
            return;
        }
        let node = &self.pool[node_idx];
        let token_str = self.token_str(node.token_id).to_string();
        let has_token = !token_str.is_empty();
        if has_token {
            prefix.push(token_str);
        }

        if node.terminal && !prefix.is_empty() {
            results.push(prefix.clone());
        }

        if cur_depth < depth {
            let cs = node.child_start as usize;
            let cc = node.child_count as usize;
            if cs != 0 || cc > 0 {
                // cs == 0 can happen if parent is root with no children
                for i in 0..cc {
                    if results.len() >= max_results {
                        break;
                    }
                    self.collect_paths(cs + i, prefix, depth, results, max_results, cur_depth + 1);
                }
            }
        }

        if has_token {
            prefix.pop();
        }
    }

    fn get_or_create_token_id(&mut self, token: &str) -> u32 {
        for (i, existing) in self.dictionary.iter().enumerate() {
            if existing == token {
                return i as u32;
            }
        }
        let id = self.dictionary.len() as u32;
        self.dictionary.push(token.to_string());
        id
    }

    pub fn tokenize_mut(&mut self, text: &str) -> Vec<u32> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| ".,;:!?\"'()[]{}<>".contains(c)))
            .filter(|w| !w.is_empty())
            .map(|w| self.get_or_create_token_id(w))
            .collect()
    }

    pub fn tokenize_path_mut(&mut self, path: &str) -> Vec<u32> {
        path.replace('\\', "/")
            .trim_start_matches('/')
            .trim_end_matches('/')
            .split('/')
            .filter(|p| !p.is_empty())
            .map(|p| self.get_or_create_token_id(p))
            .collect()
    }

    pub fn tokenize(&self, text: &str) -> Vec<u32> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| ".,;:!?\"'()[]{}<>".contains(c)))
            .filter(|w| !w.is_empty())
            .filter_map(|w| self.token_id(w))
            .collect()
    }

    pub fn tokenize_path(&self, path: &str) -> Vec<u32> {
        path.replace('\\', "/")
            .trim_start_matches('/')
            .trim_end_matches('/')
            .split('/')
            .filter(|p| !p.is_empty())
            .filter_map(|p| self.token_id(p))
            .collect()
    }

    pub fn serialize(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let node_count = self.pool.len() as u32;
        let token_count = self.dictionary.len() as u32;

        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&VERSION.to_le_bytes());
        buf.extend_from_slice(&node_count.to_le_bytes());
        buf.extend_from_slice(&token_count.to_le_bytes());

        for node in &self.pool {
            buf.extend_from_slice(&(node.token_id as i32).to_le_bytes());
            buf.extend_from_slice(&node.child_start.to_le_bytes());
            buf.extend_from_slice(&(node.child_count as i32).to_le_bytes());
            buf.push(node.terminal as u8);
        }

        for token in &self.dictionary {
            let encoded = token.as_bytes();
            buf.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            buf.extend_from_slice(encoded);
        }

        std::fs::write(path, buf)?;
        Ok(())
    }

    pub fn deserialize(path: &Path) -> anyhow::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut data)?;
        let mut offset = 0;

        if &data[offset..offset + 10] != MAGIC {
            anyhow::bail!("Bad magic");
        }
        offset += 10;

        let _version = u32::from_le_bytes(data[offset..offset + 4].try_into()?);
        offset += 4;
        let node_count = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
        offset += 4;
        let token_count = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
        offset += 4;

        let mut pool = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let token_id = i32::from_le_bytes(data[offset..offset + 4].try_into()?) as u32;
            offset += 4;
            let child_start = i32::from_le_bytes(data[offset..offset + 4].try_into()?);
            offset += 4;
            let child_count = i32::from_le_bytes(data[offset..offset + 4].try_into()?) as u32;
            offset += 4;
            let terminal = data[offset] != 0;
            offset += 1;
            pool.push(Node {
                token_id,
                child_start,
                child_count,
                terminal,
            });
        }

        let mut dictionary = Vec::with_capacity(token_count);
        for _ in 0..token_count {
            let tlen = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
            offset += 4;
            let token = String::from_utf8(data[offset..offset + tlen].to_vec())?;
            offset += tlen;
            dictionary.push(token);
        }

        Ok(Self { dictionary, pool })
    }

    pub fn export_json(&self) -> serde_json::Value {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut entities = Vec::new();
        let mut relations = Vec::new();

        for (i, node) in self.pool.iter().enumerate() {
            if node.terminal {
                let token_str = self.token_str(node.token_id);
                let path = self.path_to_root(i);
                let name = if path.is_empty() {
                    token_str.to_string()
                } else {
                    path.join(" → ")
                };

                let mut hasher = DefaultHasher::new();
                name.hash(&mut hasher);
                let eid = format!("{:016x}", hasher.finish());

                entities.push(serde_json::json!({
                    "id": eid,
                    "name": name,
                    "type": "concept",
                    "summary": token_str,
                    "tags": ["wiki"],
                }));

                if path.len() > 1 {
                    let parent_name = path[..path.len() - 1].join(" → ");
                    let mut ph = DefaultHasher::new();
                    parent_name.hash(&mut ph);
                    let parent_eid = format!("{:016x}", ph.finish());
                    relations.push(serde_json::json!({
                        "source_id": parent_eid,
                        "target_id": eid,
                        "relation": "contains",
                        "strength": 5,
                    }));
                }
            }
        }

        serde_json::json!({
            "entities": entities,
            "relations": relations,
        })
    }

    fn path_to_root(&self, node_idx: usize) -> Vec<String> {
        let mut path = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut idx = node_idx;
        while idx != 0 && !visited.contains(&idx) {
            visited.insert(idx);
            let token_str = self.token_str(self.pool[idx].token_id).to_string();
            path.push(token_str);
            match self.find_parent(idx) {
                Some(parent) => idx = parent,
                None => break,
            }
        }
        path.reverse();
        path
    }

    fn find_parent(&self, child_idx: usize) -> Option<usize> {
        for (i, node) in self.pool.iter().enumerate() {
            let cs = node.child_start as isize;
            let cc = node.child_count as usize;
            if cs >= 0 {
                let start = cs as usize;
                if (start..start + cc).contains(&child_idx) {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn stats(&self) -> HashMap<String, usize> {
        let terminal_count = self.pool.iter().filter(|n| n.terminal).count();
        let max_depth = self.compute_max_depth();
        let mut m = HashMap::new();
        m.insert("nodes".to_string(), self.pool.len());
        m.insert("tokens".to_string(), self.dictionary.len());
        m.insert("terminal_nodes".to_string(), terminal_count);
        m.insert("max_depth".to_string(), max_depth);
        m
    }

    fn compute_max_depth(&self) -> usize {
        fn dfs(pool: &[Node], idx: usize, d: usize) -> usize {
            let mut max_d = d;
            let cs = pool[idx].child_start;
            let cc = pool[idx].child_count as usize;
            if cs >= 0 {
                for i in 0..cc {
                    max_d = max_d.max(dfs(pool, cs as usize + i, d + 1));
                }
            }
            max_d
        }
        dfs(&self.pool, 0, 0)
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    pub fn merge(&mut self, other: &DynamicVectorTrie) {
        fn merge_nodes(
            pool: &[Node],
            dict: &[String],
            self_pool: &mut Vec<Node>,
            self_dict: &mut Vec<String>,
            self_idx: usize,
            other_idx: usize,
        ) {
            let cs_other = pool[other_idx].child_start as usize;
            let cc_other = pool[other_idx].child_count as usize;

            for i in 0..cc_other {
                let other_child = &pool[cs_other + i];
                let other_token_str = &dict[other_child.token_id as usize];

                let self_cs = self_pool[self_idx].child_start as usize;
                let self_cc = self_pool[self_idx].child_count as usize;
                let mut found_in_self = false;

                for j in 0..self_cc {
                    let self_child = &self_pool[self_cs + j];
                    let self_token_str = &self_dict[self_child.token_id as usize];
                    if self_token_str == other_token_str {
                        found_in_self = true;
                        if other_child.terminal {
                            self_pool[self_cs + j].terminal = true;
                        }
                        merge_nodes(pool, dict, self_pool, self_dict, self_cs + j, cs_other + i);
                        break;
                    }
                }

                if !found_in_self {
                    let child_tid = {
                        let mut found = None;
                        for (ti, t) in self_dict.iter().enumerate() {
                            if t == other_token_str {
                                found = Some(ti as u32);
                                break;
                            }
                        }
                        match found {
                            Some(tid) => tid,
                            None => {
                                let tid = self_dict.len() as u32;
                                self_dict.push(other_token_str.clone());
                                tid
                            }
                        }
                    };

                    let child_idx = {
                        let new_cs = self_pool.len() as i32;
                        for j in 0..self_cc {
                            let child = self_pool[self_cs + j].clone();
                            self_pool.push(child);
                        }
                        let idx = self_pool.len();
                        self_pool.push(Node {
                            token_id: child_tid,
                            child_start: -1,
                            child_count: 0,
                            terminal: other_child.terminal,
                        });
                        self_pool[self_idx].child_start = new_cs;
                        self_pool[self_idx].child_count += 1;
                        idx
                    };

                    merge_nodes(pool, dict, self_pool, self_dict, child_idx, cs_other + i);
                }
            }
        }

        merge_nodes(
            &other.pool,
            &other.dictionary,
            &mut self.pool,
            &mut self.dictionary,
            0,
            0,
        );
    }

    pub fn compute_diff(&self, other: &DynamicVectorTrie) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
        let mut added = Vec::new();
        let mut removed = Vec::new();

        fn enumerate_paths(pool: &[Node], dict: &[String], idx: usize, prefix: &mut Vec<String>, results: &mut Vec<Vec<String>>) {
            let node = &pool[idx];
            if node.token_id != 0 || idx != 0 {
                let ts = dict[node.token_id as usize].clone();
                prefix.push(ts);
            }
            if node.terminal && !prefix.is_empty() {
                results.push(prefix.clone());
            }
            let cs = node.child_start as usize;
            let cc = node.child_count as usize;
            for i in 0..cc {
                enumerate_paths(pool, dict, cs + i, prefix, results);
            }
            if (node.token_id != 0 || idx != 0) && !prefix.is_empty() {
                prefix.pop();
            }
        }

        let mut self_paths = Vec::new();
        let mut other_paths = Vec::new();
        let mut prefix = Vec::new();
        enumerate_paths(&self.pool, &self.dictionary, 0, &mut prefix, &mut self_paths);
        prefix.clear();
        enumerate_paths(&other.pool, &other.dictionary, 0, &mut prefix, &mut other_paths);

        for path in &other_paths {
            if !self_paths.contains(path) {
                added.push(path.clone());
            }
        }
        for path in &self_paths {
            if !other_paths.contains(path) {
                removed.push(path.clone());
            }
        }

        (added, removed)
    }
}

impl Default for DynamicVectorTrie {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_search() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("project/philosophy/identity");
        let tids = t.tokenize_path("project/philosophy/identity");
        assert!(t.search(&tids));
        let fake = t.tokenize_path("project/philosophy/fake");
        assert!(fake.is_empty() || !t.search(&fake));
    }

    #[test]
    fn test_prefix_search() {
        let mut t = DynamicVectorTrie::new();
        for s in &[
            "project/philosophy/identity",
            "project/philosophy/recursion",
            "project/ai/memory-compression",
            "project/ai/vector-trie",
        ] {
            t.insert_path(s);
        }

        let results = t.prefix_search_str(&["project"], 2, 10);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let mut t = DynamicVectorTrie::new();
        t.insert_str("hello world");
        t.insert_str("hello rust");
        let tmp = std::env::temp_dir().join("test_trie.bin");
        t.serialize(&tmp).unwrap();
        let t2 = DynamicVectorTrie::deserialize(&tmp).unwrap();
        assert_eq!(t.pool.len(), t2.pool.len());
        assert_eq!(t.dictionary, t2.dictionary);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_delete() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("test/path");
        let tokens = t.tokenize_path("test/path");
        assert!(t.search(&tokens));
        assert!(t.delete(&tokens));
        assert!(!t.search(&tokens));
        assert!(!t.delete(&tokens));
    }

    #[test]
    fn test_constant_cost() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("project/feature");
        let cost_1 = t.prefix_search_str(&["project"], 2, 8).len();
        for i in 0..1000u32 {
            t.insert_path(&format!("otherbranch/sub-{i}/leaf"));
        }
        let cost_2 = t.prefix_search_str(&["project"], 2, 8).len();
        assert_eq!(cost_1, cost_2, "query cost should not grow with unrelated trie size");
    }

    #[test]
    fn test_merge() {
        let mut t1 = DynamicVectorTrie::new();
        t1.insert_path("a/b/c");
        let mut t2 = DynamicVectorTrie::new();
        t2.insert_path("a/b/d");
        t1.merge(&t2);
        assert!(t1.search(&t1.tokenize_path("a/b/c")));
        assert!(t1.search(&t1.tokenize_path("a/b/d")));
    }

    #[test]
    fn test_diff() {
        let mut t1 = DynamicVectorTrie::new();
        t1.insert_path("a/b/c");
        let mut t2 = DynamicVectorTrie::new();
        t2.insert_path("a/b/c");
        t2.insert_path("a/b/d");
        let (added, removed) = t1.compute_diff(&t2);
        assert_eq!(added.len(), 1);
        assert_eq!(removed.len(), 0);
    }

    #[test]
    fn test_stats() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("a/b/c");
        t.insert_path("a/b/d");
        let s = t.stats();
        assert_eq!(*s.get("nodes").unwrap(), 6);
        assert_eq!(*s.get("terminal_nodes").unwrap(), 3);
    }

    #[test]
    fn test_empty_trie_search() {
        let t = DynamicVectorTrie::new();
        assert!(!t.search(&[]));
        let tids = t.tokenize_path("anything");
        assert!(tids.is_empty() || !t.search(&tids));
    }

    #[test]
    fn test_empty_trie_serialize() {
        let t = DynamicVectorTrie::new();
        let tmp = std::env::temp_dir().join("test_empty_trie.bin");
        t.serialize(&tmp).unwrap();
        let t2 = DynamicVectorTrie::deserialize(&tmp).unwrap();
        assert_eq!(t.pool.len(), t2.pool.len());
        assert_eq!(t.dictionary, t2.dictionary);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_single_node_diff() {
        let t1 = DynamicVectorTrie::new();
        let t2 = DynamicVectorTrie::new();
        let (added, removed) = t1.compute_diff(&t2);
        assert_eq!(added.len(), 0);
        assert_eq!(removed.len(), 0);
    }

    #[test]
    fn test_merge_into_self() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("a/b/c");
        let clone_t = {
            let mut c = DynamicVectorTrie::new();
            c.insert_path("a/b/c");
            c
        };
        t.merge(&clone_t);
        assert!(t.search(&t.tokenize_path("a/b/c")));
    }

    #[test]
    fn test_large_path() {
        let mut t = DynamicVectorTrie::new();
        let path = "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p";
        t.insert_path(path);
        let tids = t.tokenize_path(path);
        assert!(t.search(&tids));
    }

    #[test]
    fn test_prefix_search_no_match() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("project/feature");
        let results = t.prefix_search_str(&["nonexistent"], 2, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_multiple_insert_same_path() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("same/path");
        t.insert_path("same/path");
        let tids = t.tokenize_path("same/path");
        assert!(t.search(&tids));
    }

    #[test]
    fn test_insert_empty_string() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("");
        assert_eq!(t.pool.len(), 1);
    }

    #[test]
    fn test_export_json_empty() {
        let t = DynamicVectorTrie::new();
        let json = t.export_json();
        assert_eq!(json["entities"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_export_json_with_data() {
        let mut t = DynamicVectorTrie::new();
        t.insert_path("a/b/c");
        let json = t.export_json();
        assert!(!json["entities"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_walk_non_existent() {
        let t = DynamicVectorTrie::new();
        assert!(t.walk(&[999]).is_none());
    }
}
