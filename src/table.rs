//  This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub const RED: &str = "\u{1b}[1;31m";
pub const GREEN: &str = "\u{1b}[1;32m";
pub const YELLOW: &str = "\u{1b}[1;33m";
pub const BOLD: &str = "\u{1b}[1;37m";
const MUTED: &str = "\u{1b}[2;37m";
const RESET: &str = "\u{1b}[0m";

pub struct Table {
	i: usize,
	j: usize,
	tab: Vec<Vec<String>>,
	col: Vec<usize>,
	mult: usize,
}

impl Table {
	pub fn new() -> Table {
		Table {
			i: 0,
			j: 0,
			tab: vec![],
			col: vec![],
			mult: 1,
		}
	}

	fn row(&mut self) -> &mut Table {
		self.i += 1;
		self.j = 0;
		self.tab.push(vec![]);
		self
	}

	fn col(&mut self, len: usize) {
		if self.col.len() <= self.j {
			self.col.push(0);
		}
		if self.col[self.j] < len {
			self.col[self.j] = len;
		}
		self.j += 1;
	}

	fn txt(&mut self, txt: String) {
		self.tab[self.i-1].push(txt);
	}

	pub fn field(&mut self, txt: &str, color: &str) -> &mut Table {
		self.txt(format!("{color}{txt}{RESET}"));
		self.col(txt.len());
		self
	}

	pub fn first(&mut self, txt: &str) -> &mut Table {
		self.row();
		self.field(txt, BOLD)
	}

	pub fn ppfirst(&mut self, pre: &str, txt: &str, post: &str) -> &mut Table {
		self.mult = 3;
		self.row();
		self.txt(format!("{MUTED}{pre}{RESET}{BOLD}{txt}{RESET}{MUTED}{post}{RESET}"));
		self.col(pre.len() + txt.len() + post.len());
		self
	}

	pub fn empty(&mut self, num: u8) -> &mut Table {
		for _ in 0 .. num {
			self.field("", BOLD);
		}
		self
	}

	pub fn print(&self) {
		let mut table = String::from('\n');

		for row in &self.tab {
			let indent = self.col[0] + 11 * self.mult;
			let line = format!("  {:<indent$}", row[0]);
			table.push_str(&line);

			for (j, txt) in row.iter().enumerate().skip(1) {
				let indent = self.col[j] + 11;
				let line = format!(" {MUTED}|{RESET} {:<indent$}", txt);
				table.push_str(&line);
			}

			table.push('\n');
		}

		println!("{table}");
	}
}

pub fn table_err(first: &str, txt: &str) {
	let mut table = Table::new();
	table.first(first).field(txt, RED).print();
}

pub fn log_bold(first: &str, txt: &str) {
	println!("{MUTED}{first}{RESET} {BOLD}{txt}{RESET} {MUTED}...{RESET}");
}
