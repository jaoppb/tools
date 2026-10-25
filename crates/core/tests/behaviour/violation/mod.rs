//! The findings: how they print and how they sort.

use tools_core::domain::{Cycle, Rule, Violation, Violations};

use crate::support::number;

fn printed(violations: &Violations) -> Vec<String> {
    violations.iter().map(ToString::to_string).collect()
}

#[test]
fn prints_a_violation_as_number_slug_and_message() {
    let violation = Violation::new(number(97), Rule::NoArea);
    assert_eq!(violation.to_string(), "#97 no-area: no area label");
}

#[test]
fn prints_the_other_issue_where_the_line_needs_it() {
    let closed = Violation::new(number(97), Rule::ClosedParent(number(91)));
    assert_eq!(
        closed.to_string(),
        "#97 closed-parent: open under the closed parent #91"
    );
    let blocks = Violation::new(number(5), Rule::BlocksEarlierRelease(number(6)));
    assert_eq!(
        blocks.to_string(),
        "#5 blocks-earlier-release: blocks #6, which is in an earlier release"
    );
}

#[test]
fn sorts_violations_by_issue_number_then_rule() {
    let violations = Violations::new(vec![
        Violation::new(number(9), Rule::NoArea),
        Violation::new(number(3), Rule::NoMilestone),
        Violation::new(number(3), Rule::NotOneType),
    ]);
    assert_eq!(
        printed(&violations),
        [
            "#3 not-one-type: not exactly one type label",
            "#3 no-milestone: no milestone",
            "#9 no-area: no area label",
        ]
    );
}

#[test]
fn names_every_member_of_a_cycle_in_its_line() {
    let cycle = Cycle::new(vec![number(10), number(11), number(12)]);
    let violation = Violation::new(number(10), Rule::Cycle(cycle));
    assert_eq!(
        violation.to_string(),
        "#10 blocked-by-cycle: blocked-by cycle: #10, #11, #12"
    );
}
