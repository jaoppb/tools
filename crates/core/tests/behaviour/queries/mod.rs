//! The GraphQL requests, read from their files and filled in.

use tools_core::adapters::github::Target;
use tools_core::adapters::github::query;

#[test]
fn every_filled_query_has_no_placeholder_left() {
    let target = Target::cosmo();
    let queries = [
        query::items(&target),
        query::pulls(&target),
        query::item_of_issue(&target, 20),
        query::status_field(&target),
        query::repo_labels(&target),
        query::repo_milestones(&target),
    ];
    assert!(queries.iter().all(|text| !text.contains('@')));
}

#[test]
fn a_filled_query_names_the_owner_and_the_project() {
    let target = Target::cosmo();
    assert!(query::items(&target).contains("login: \"jaoppb\""));
    assert!(query::items(&target).contains("projectV2(number: 4)"));
    assert!(query::pulls(&target).contains("name: \"cosmo\""));
    assert!(query::item_of_issue(&target, 20).contains("issue(number: 20)"));
    for variable in ["$project", "$item", "$field", "$option"] {
        assert!(query::SET_STATUS.contains(variable));
    }
}
