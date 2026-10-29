//! The release a milestone title names.

use tools_core::domain::{Milestone, Release};

fn release(title: &str) -> Option<Release> {
    Release::of(&Milestone::new(title))
}

fn later(title: &str, other: &str) -> bool {
    let (Some(first), Some(second)) = (release(title), release(other)) else {
        return false;
    };
    first.is_later_than(&second)
}

#[test]
fn reads_v1_as_release_one_zero() {
    assert!(release("v1").is_some());
    assert_eq!(release("v1"), release("v1.0"));
    assert_eq!(release("v1"), release("v1.0.0"));
}

#[test]
fn reads_a_title_with_three_numbers() {
    assert!(release("v0.1.0").is_some());
    assert_eq!(release("v0.1.0"), release("v0.1"));
    assert!(later("v0.2.0", "v0.1.0"));
    assert!(later("v0.1.1", "v0.1.0"));
    assert!(!later("v0.1.0", "v0.1.1"));
}

#[test]
fn reads_v1_1_as_a_later_release_than_v1() {
    assert!(later("v1.1", "v1"));
    assert!(!later("v1", "v1.1"));
}

#[test]
fn reads_v0_as_the_earliest_release() {
    assert!(later("v1", "v0"));
    assert!(!later("v0", "v0"));
}

#[test]
fn orders_v1_10_after_v1_2() {
    assert!(later("v1.10", "v1.2"));
}

#[test]
fn orders_v2_after_v1_9() {
    assert!(later("v2", "v1.9"));
}

#[test]
fn rejects_a_title_without_a_v() {
    assert_eq!(release("1.1"), None);
}

#[test]
fn rejects_a_title_with_four_numbers() {
    assert_eq!(release("v1.1.2.3"), None);
}

#[test]
fn rejects_a_title_with_text_or_a_sign() {
    assert_eq!(release("backlog"), None);
    assert_eq!(release("v1.x"), None);
    assert_eq!(release("v+1"), None);
    assert_eq!(release("v1."), None);
    assert_eq!(release("v0.1."), None);
    assert_eq!(release("v0..1"), None);
    assert_eq!(release("v.1.0"), None);
    assert_eq!(release("v"), None);
}
