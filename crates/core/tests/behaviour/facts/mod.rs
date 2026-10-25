//! The small value objects of the domain.

use tools_core::domain::{Blocker, Blockers, Effort, Label, Labels, Lifecycle, Status};

use crate::support::number;

fn labels(names: &[&str]) -> Labels {
    Labels::new(names.iter().map(|name| Label::new(name)).collect())
}

#[test]
fn labels_need_exactly_one_type_label() {
    assert!(labels(&["type:feat", "area:x"]).has_one_type());
    assert!(!labels(&["area:x"]).has_one_type());
    assert!(!labels(&["type:feat", "type:fix"]).has_one_type());
}

#[test]
fn labels_need_an_area_label() {
    assert!(labels(&["type:feat", "area:app-ui"]).has_area());
    assert!(!labels(&["type:feat"]).has_area());
}

#[test]
fn an_effort_of_eight_fits_ready_and_nine_does_not() {
    assert!(Effort::new(8).fits_ready());
    assert!(!Effort::new(9).fits_ready());
}

#[test]
fn no_blockers_counts_as_all_closed() {
    assert!(Blockers::new(Vec::new()).all_closed());
    let open = Blockers::new(vec![Blocker::new(number(1), Lifecycle::Open)]);
    assert!(!open.all_closed());
}

#[test]
fn an_issue_number_prints_with_a_hash() {
    assert_eq!(number(92).to_string(), "#92");
}

#[test]
fn status_names_match_the_project_options() {
    let names = ["Backlog", "Ready", "In progress", "In review", "Done"];
    let statuses = names.map(Status::from_name);
    assert!(statuses.iter().all(Option::is_some));
    assert_eq!(Status::from_name("In progress"), Some(Status::InProgress));
    assert_eq!(Status::from_name("Doing"), None);
}
