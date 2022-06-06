use std::time::Instant;

pub struct OpenTimer<'a> {
    name: &'static str,
    timer_tree: &'a mut TimerTree,
    start: Instant,
    depth: u32,
}

impl<'a> OpenTimer<'a> {
    /// Starts timing a new named subtask
    ///
    /// The timer is stopped automatically
    /// when the `OpenTimer` is dropped.
    pub fn open(&mut self, name: &'static str) -> OpenTimer<'_> {
        OpenTimer {
            name,
            timer_tree: self.timer_tree,
            start: Instant::now(),
            depth: self.depth + 1,
        }
    }
}

impl<'a> Drop for OpenTimer<'a> {
    fn drop(&mut self) {
        self.timer_tree.timings.push(Timing {
            name: self.name,
            duration: self.start.elapsed().as_micros(),
            depth: self.depth,
        });
    }
}

/// Timing recording
#[derive(Debug, serde::Serialize)]
pub struct Timing {
    name: &'static str,
    duration: u128,
    depth: u32,
}

/// Timer tree
#[derive(Debug, Default, serde::Serialize)]
pub struct TimerTree {
    timings: Vec<Timing>,
}

impl TimerTree {
    /// Returns the total time elapsed in microseconds
    pub fn total_time(&self) -> u128 {
        self.timings.last().unwrap().duration
    }

    /// Open a new named subtask
    pub fn open(&mut self, name: &'static str) -> OpenTimer<'_> {
        OpenTimer {
            name,
            timer_tree: self,
            start: Instant::now(),
            depth: 0,
        }
    }
}
