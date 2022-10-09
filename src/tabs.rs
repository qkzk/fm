use crate::{args::Args, config::Config, status::Status};

pub struct Tabs {
    pub statuses: Vec<Status>,
    pub index: usize,
}

impl Tabs {
    pub fn new(args: Args, config: Config, height: usize) -> Self {
        let status = Status::new(args, config, height);

        Self {
            statuses: vec![status.clone(), status],
            index: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    pub fn len(&self) -> usize {
        self.statuses.len()
    }

    pub fn next(&mut self) {
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.len()
        }
    }

    pub fn prev(&mut self) {
        if self.is_empty() {
            self.index = 0
        } else if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.len() - 1
        }
    }

    pub fn selected(&mut self) -> &mut Status {
        &mut self.statuses[self.index]
    }
}
