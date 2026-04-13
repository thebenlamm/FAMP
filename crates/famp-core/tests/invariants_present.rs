//! Guards against silent deletion of the INV-1..INV-11 documentation anchors.
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

const SRC: &str = include_str!("../src/invariants.rs");

#[test]
fn eleven_constants_present() {
    let count = SRC
        .lines()
        .filter(|l| l.trim_start().starts_with("pub const INV_"))
        .count();
    assert_eq!(count, 11);
}

#[test]
fn eleven_doc_comments_nonempty() {
    let lines: Vec<&str> = SRC.lines().collect();
    let mut checked = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("pub const INV_") {
            // Walk backwards skipping any wrapped `///` lines to find the
            // doc-comment block; the nearest preceding non-empty line must be
            // a `///` line.
            let prev = lines.get(i.wrapping_sub(1)).copied().unwrap_or("");
            assert!(
                prev.trim_start().starts_with("///"),
                "missing doc comment above {line}"
            );
            // The doc-comment *block* (concatenated) must be substantive.
            // Walk back collecting all contiguous `///` lines.
            let mut block = String::new();
            let mut j = i;
            while j > 0 {
                j -= 1;
                let l = lines[j].trim_start();
                if let Some(rest) = l.strip_prefix("///") {
                    block.insert_str(0, rest.trim());
                    block.insert(0, ' ');
                } else {
                    break;
                }
            }
            assert!(
                block.trim().len() > 20,
                "doc comment too short above {line}: {block:?}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 11);
}

#[test]
fn constants_resolve() {
    use famp_core::invariants::{
        INV_1, INV_10, INV_11, INV_2, INV_3, INV_4, INV_5, INV_6, INV_7, INV_8, INV_9,
    };
    assert_eq!(INV_1, "INV-1");
    assert_eq!(INV_2, "INV-2");
    assert_eq!(INV_3, "INV-3");
    assert_eq!(INV_4, "INV-4");
    assert_eq!(INV_5, "INV-5");
    assert_eq!(INV_6, "INV-6");
    assert_eq!(INV_7, "INV-7");
    assert_eq!(INV_8, "INV-8");
    assert_eq!(INV_9, "INV-9");
    assert_eq!(INV_10, "INV-10");
    assert_eq!(INV_11, "INV-11");
}

#[test]
fn constants_are_distinct() {
    use famp_core::invariants::{
        INV_1, INV_10, INV_11, INV_2, INV_3, INV_4, INV_5, INV_6, INV_7, INV_8, INV_9,
    };
    let all = [
        INV_1, INV_2, INV_3, INV_4, INV_5, INV_6, INV_7, INV_8, INV_9, INV_10, INV_11,
    ];
    let mut sorted: Vec<&str> = all.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 11);
}
