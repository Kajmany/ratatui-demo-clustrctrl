//! Widget for generating candidate tasks from a big list. Keeps full Ratatui list state, but
//! we only care about the cursor, really . Not responsible for actually making tasks

use core::fmt;

use rand::seq::IndexedRandom;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, StatefulWidget, Widget},
};

/// How many entries to pick out for the menu
const FETCH_AMOUNT: usize = 6;

#[derive(Debug)]
pub struct TaskPicker {
    items: Vec<&'static CandidateTask>,
    pub state: ListState,
}

#[derive(Debug)]
pub struct CandidateTask {
    pub name: &'static str,
    pub description: &'static str,
}

impl fmt::Display for CandidateTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}): {}", self.name, self.description)
    }
}

impl Default for TaskPicker {
    fn default() -> Self {
        Self {
            items: gen_list(),
            state: ListState::default().with_selected(Some((FETCH_AMOUNT / 2) - 1)),
        }
    }
}

impl TaskPicker {
    /// Wraps list down
    pub fn next(&mut self) {
        self.state.select_next();
    }

    /// Wraps list up
    pub fn previous(&mut self) {
        self.state.select_previous();
    }

    /// Should be called on state change FROM modal, to get candidate for creation in main
    pub fn select(&self) -> Option<&'static CandidateTask> {
        // This SHOULD always have something selected, but we will handle the possibility back in main
        // Return None if there's no selection. Return None if there is and no item @ selection
        if let Some(idx) = self.state.selected() {
            self.items.get(idx).map(|ct| Some(*ct))?
        } else {
            None
        }
    }

    /// Should be called every time the modal is 'opened' (state change in main). Picks from the
    /// pool and rebuilds list again
    pub fn regen(&mut self) {
        self.items = COOL_TASKS
            .choose_multiple(&mut rand::rng(), FETCH_AMOUNT)
            .collect()
    }
}

impl Widget for &mut TaskPicker {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        //TODO: Style me!
        let styled_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| ListItem::from(item.to_string()))
            .collect();

        let block = Block::new().title(" New Task ").borders(Borders::ALL);

        let list = List::new(styled_items).block(block).highlight_symbol(">");
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}

fn gen_list() -> Vec<&'static CandidateTask> {
    COOL_TASKS
        .choose_multiple(&mut rand::rng(), FETCH_AMOUNT)
        .collect()
}

const COOL_TASKS: &[CandidateTask] = &[
    CandidateTask {
        name: "Bobson Dugnutt",
        description: "Wait for Pokemon cards",
    },
    CandidateTask {
        name: "Sleve McDichael",
        description: "Re-attach turbo encabulator",
    },
    CandidateTask {
        name: "Onson Sweemey",
        description: "Repaint fence",
    },
    CandidateTask {
        name: "Anatoli Smorin",
        description: "Revandalize fence",
    },
    CandidateTask {
        name: "Rey McSriff",
        description: "help im trapped in a binary an",
    },
    CandidateTask {
        name: "Glenallen Mixon",
        description: "Rehydrate the PDF files",
    },
    CandidateTask {
        name: "Mario McRlwain",
        description: "Defragment rubber duck collection",
    },
    CandidateTask {
        name: "Todd Bonzalez",
        description: "Uninstall gravity temporarily",
    },
    CandidateTask {
        name: "Dwigt Rortugal",
        description: "Calibrate the hydrospanner flux matrix",
    },
    CandidateTask {
        name: "Karl Dandleton",
        description: "Reverse-engineer cafeteria meatloaf",
    },
    CandidateTask {
        name: "Mike Truk",
        description: "Overclock the toaster (bagels only)",
    },
    CandidateTask {
        name: "Dean Wesrey",
        description: "Re-enact fax machine error codes via mime",
    },
    CandidateTask {
        name: "Raul Chamgerlain",
        description: "Translate whale songs into Excel formulas",
    },
    CandidateTask {
        name: "Tony Smellme",
        description: "Teach office plants about blockchain",
    },
    CandidateTask {
        name: "Jeromy Gride",
        description: "Recycle the same oxygen molecule 17 times",
    },
];
