//! One page of the Project's items, read into cards.

mod shapes;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::bad_reply;
use crate::adapters::github::parse::items::shapes::{
    Blocking, Content, Detail, FieldValues, Item, Names, Parented, Reply,
};
use crate::adapters::github::target::Target;
use crate::domain::{
    Blocker, Blockers, Card, Effort, Facts, IssueNumber, Label, Labels, Lifecycle, Links,
    Milestone, Parent, Priority, Role, Standing, Status,
};

/// The cards of one page and the cursor of the next page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemsPage {
    cards: Vec<Card>,
    next: Option<String>,
}

impl ItemsPage {
    /// The cards of this page.
    #[must_use]
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// The cursor to pass for the next page, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }
}

/// Reads one page of the Project's items. Items that are not issues of the
/// target repository are dropped.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON, and
/// [`GhError::UnknownStatus`] when an item has a Status this tool does not know.
pub fn parse_items(json: &str, target: &Target) -> Result<ItemsPage, GhError> {
    let reply: Reply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let items = reply.data.user.project_v2.items;
    let next = items.page_info.next();
    let cards = items
        .nodes
        .iter()
        .map(|item| card_of(item, target))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ItemsPage {
        cards: cards.into_iter().flatten().collect(),
        next,
    })
}

fn card_of(item: &Item, target: &Target) -> Result<Option<Card>, GhError> {
    let Some(content) = &item.content else {
        return Ok(None);
    };
    let Some(raw) = content.head.number else {
        return Ok(None);
    };
    if !content.head.is_in(target) {
        return Ok(None);
    }
    let card = Card::new(IssueNumber::new(raw), standing_of(item, content)?)
        .with_facts(facts_of(&item.field_values, &content.detail))
        .with_links(links_of(&content.detail, target));
    Ok(Some(card))
}

fn standing_of(item: &Item, content: &Content) -> Result<Standing, GhError> {
    let status = match item.field_values.select("Status") {
        None => Status::Backlog,
        Some(name) => Status::from_name(name).ok_or_else(|| GhError::UnknownStatus {
            name: name.to_owned(),
        })?,
    };
    Ok(Standing::new(
        status,
        lifecycle_of(content.head.state.as_deref())?,
    ))
}

fn lifecycle_of(state: Option<&str>) -> Result<Lifecycle, GhError> {
    match state {
        Some("OPEN") => Ok(Lifecycle::Open),
        Some("CLOSED") => Ok(Lifecycle::Closed),
        other => Err(GhError::BadReply {
            reason: format!("an issue has the state {other:?}"),
        }),
    }
}

fn labels_of(names: Option<&Names>) -> Labels {
    let labels = names
        .into_iter()
        .flat_map(|names| &names.nodes)
        .map(|named| Label::new(&named.name))
        .collect();
    Labels::new(labels)
}

fn facts_of(values: &FieldValues, detail: &Detail) -> Facts {
    let facts = Facts::new(labels_of(detail.labels.as_ref()));
    with_effort(with_priority(with_milestone(facts, detail), values), values)
}

fn with_milestone(facts: Facts, detail: &Detail) -> Facts {
    match &detail.milestone {
        Some(milestone) => facts.with_milestone(Milestone::new(&milestone.title)),
        None => facts,
    }
}

fn with_priority(facts: Facts, values: &FieldValues) -> Facts {
    match values.select("Priority") {
        Some(name) => facts.with_priority(Priority::new(name)),
        None => facts,
    }
}

fn with_effort(facts: Facts, values: &FieldValues) -> Facts {
    match values.whole_number("Effort") {
        Some(points) => facts.with_effort(Effort::new(points)),
        None => facts,
    }
}

fn links_of(detail: &Detail, target: &Target) -> Links {
    let blockers = detail
        .blocked_by
        .iter()
        .flat_map(|blocked| &blocked.nodes)
        .filter_map(|blocking| blocker_of(blocking, target))
        .collect();
    let links = Links::new(Blockers::new(blockers));
    match &detail.parent {
        Some(parent) => links.with_parent(parent_of(parent)),
        None => links,
    }
}

/// A blocker from another repository, or one whose state is not closed, is
/// read as open, so nothing is promoted by mistake.
fn blocker_of(blocking: &Blocking, target: &Target) -> Option<Blocker> {
    let number = IssueNumber::new(blocking.head.number?);
    let closed = blocking.head.is_in(target) && blocking.head.state.as_deref() == Some("CLOSED");
    let lifecycle = if closed {
        Lifecycle::Closed
    } else {
        Lifecycle::Open
    };
    Some(Blocker::new(number, lifecycle))
}

fn parent_of(parent: &Parented) -> Parent {
    let epic = parent.labels.nodes.iter().any(|named| named.name == "epic");
    let role = if epic { Role::Epic } else { Role::Story };
    Parent::new(IssueNumber::new(parent.number), role)
}
