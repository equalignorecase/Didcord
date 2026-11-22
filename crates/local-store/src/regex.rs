//! Bounded linear-time PikeVM regex engine for local cache search (zero ReDoS, no unsafe code).

const MAX_PATTERN_BYTES: usize = 256;
const MAX_INSTRUCTIONS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegexError;

#[derive(Clone, Debug)]
enum ClassAtom {
	Char(char),
	Range(char, char),
	Digit,
	Word,
	Space,
}

#[derive(Clone, Debug)]
enum Matcher {
	Any,
	Char(char),
	Class {
		negated: bool,
		items: Vec<ClassAtom>,
	},
	Digit(bool),
	Word(bool),
	Space(bool),
}

impl Matcher {
	fn matches(&self, ch: char, case_insensitive: bool) -> bool {
		let eq = |a: char, b: char| {
			if case_insensitive {
				a.eq_ignore_ascii_case(&b) || a.to_lowercase().eq(b.to_lowercase())
			} else {
				a == b
			}
		};
		match self {
			Self::Any => ch != '\n',
			Self::Char(c) => eq(ch, *c),
			Self::Digit(neg) => ch.is_ascii_digit() ^ *neg,
			Self::Word(neg) => (ch.is_alphanumeric() || ch == '_') ^ *neg,
			Self::Space(neg) => ch.is_whitespace() ^ *neg,
			Self::Class { negated, items } => {
				let hit = items.iter().any(|item| match item {
					ClassAtom::Char(c) => eq(ch, *c),
					ClassAtom::Range(lo, hi) => {
						if case_insensitive {
							let lc = ch.to_ascii_lowercase();
							lc >= lo.to_ascii_lowercase() && lc <= hi.to_ascii_lowercase()
						} else {
							ch >= *lo && ch <= *hi
						}
					}
					ClassAtom::Digit => ch.is_ascii_digit(),
					ClassAtom::Word => ch.is_alphanumeric() || ch == '_',
					ClassAtom::Space => ch.is_whitespace(),
				});
				hit ^ *negated
			}
		}
	}
}

#[derive(Clone, Debug)]
enum Inst {
	Match(Matcher),
	AnchorStart,
	AnchorEnd,
	Jump(usize),
	Split(usize, usize),
	Accept,
}

pub(crate) struct Regex {
	program: Vec<Inst>,
	case_insensitive: bool,
}

impl Regex {
	pub(crate) fn compile(mut pattern: &str) -> Result<Self, RegexError> {
		if pattern.is_empty() || pattern.len() > MAX_PATTERN_BYTES {
			return Err(RegexError);
		}
		let mut case_insensitive = false;
		if let Some(rest) = pattern.strip_prefix("(?i)") {
			case_insensitive = true;
			pattern = rest;
		}
		if pattern.is_empty() {
			return Err(RegexError);
		}
		let chars: Vec<char> = pattern.chars().collect();
		let mut parser = Parser {
			chars: &chars,
			pos: 0,
			program: Vec::new(),
		};
		let entry = parser.parse_alt_clean()?;
		if parser.pos != chars.len() {
			return Err(RegexError);
		}
		parser.emit(Inst::Accept)?;
		let _ = entry;
		Ok(Self {
			program: parser.program,
			case_insensitive,
		})
	}

	pub(crate) fn is_match(&self, text: &str) -> bool {
		let chars: Vec<char> = text.chars().collect();
		let n = self.program.len();
		let mut active = Vec::with_capacity(n);
		let mut next = Vec::with_capacity(n);
		let mut seen = vec![0_usize; n];
		let mut gen_id = 1_usize;

		for pos in 0..=chars.len() {
			// Unanchored match: start a new candidate thread at each position unless already active.
			gen_id += 1;
			self.add_thread(&mut active, &mut seen, gen_id, 0, pos, chars.len());
			if active
				.iter()
				.any(|&pc| matches!(self.program[pc], Inst::Accept))
			{
				return true;
			}
			if pos == chars.len() {
				break;
			}
			let ch = chars[pos];
			next.clear();
			gen_id += 1;
			for &pc in &active {
				if let Inst::Match(matcher) = &self.program[pc]
					&& matcher.matches(ch, self.case_insensitive)
				{
					self.add_thread(&mut next, &mut seen, gen_id, pc + 1, pos + 1, chars.len());
				}
			}
			if next
				.iter()
				.any(|&pc| matches!(self.program[pc], Inst::Accept))
			{
				return true;
			}
			std::mem::swap(&mut active, &mut next);
			// Keep active threads from previous start positions too: wait, reusing `active`
			// plus adding `pc = 0` at `pos + 1` in the loop handles all active partial matches!
		}
		false
	}

	fn add_thread(
		&self,
		list: &mut Vec<usize>,
		seen: &mut [usize],
		gen_id: usize,
		pc: usize,
		pos: usize,
		len: usize,
	) {
		let mut stack = vec![pc];
		while let Some(cur) = stack.pop() {
			if cur >= self.program.len() || seen[cur] == gen_id {
				continue;
			}
			seen[cur] = gen_id;
			match &self.program[cur] {
				Inst::Jump(target) => stack.push(*target),
				Inst::Split(a, b) => {
					stack.push(*b);
					stack.push(*a);
				}
				Inst::AnchorStart => {
					if pos == 0 {
						stack.push(cur + 1);
					}
				}
				Inst::AnchorEnd => {
					if pos == len {
						stack.push(cur + 1);
					}
				}
				Inst::Match(_) | Inst::Accept => list.push(cur),
			}
		}
	}
}

struct Parser<'a> {
	chars: &'a [char],
	pos: usize,
	program: Vec<Inst>,
}

impl Parser<'_> {
	fn emit(&mut self, inst: Inst) -> Result<usize, RegexError> {
		if self.program.len() >= MAX_INSTRUCTIONS {
			return Err(RegexError);
		}
		let idx = self.program.len();
		self.program.push(inst);
		Ok(idx)
	}

	fn peek(&self) -> Option<char> {
		self.chars.get(self.pos).copied()
	}

	fn next_char(&mut self) -> Option<char> {
		let ch = self.peek()?;
		self.pos += 1;
		Some(ch)
	}

	fn parse_concat(&mut self) -> Result<usize, RegexError> {
		let start = self.program.len();
		while let Some(ch) = self.peek() {
			if matches!(ch, '|' | ')') {
				break;
			}
			self.parse_quantified()?;
		}
		Ok(start)
	}

	fn parse_quantified(&mut self) -> Result<(), RegexError> {
		let atom_start = self.program.len();
		self.parse_atom()?;
		if let Some(q @ ('*' | '+' | '?')) = self.peek() {
			self.next_char();
			let after = self.program.len();
			match q {
				'?' => {
					// Insert Split(atom_start + 1, after + 1) by shifting atom instructions.
					self.insert_inst(atom_start, Inst::Split(atom_start + 1, after + 1))?;
				}
				'*' => {
					let split = Inst::Split(atom_start + 1, after + 2);
					self.insert_inst(atom_start, split)?;
					self.emit(Inst::Jump(atom_start))?;
				}
				'+' => {
					self.emit(Inst::Split(atom_start, after + 1))?;
				}
				_ => {}
			}
		}
		Ok(())
	}

	fn insert_inst(&mut self, at: usize, inst: Inst) -> Result<(), RegexError> {
		if self.program.len() >= MAX_INSTRUCTIONS {
			return Err(RegexError);
		}
		self.program.insert(at, inst);
		// Shift jump targets inside the moved block [at + 1 ..].
		for item in &mut self.program[at + 1..] {
			match item {
				Inst::Jump(t) if *t >= at => *t += 1,
				Inst::Split(a, b) => {
					if *a >= at {
						*a += 1;
					}
					if *b >= at {
						*b += 1;
					}
				}
				_ => {}
			}
		}
		Ok(())
	}

	fn parse_atom(&mut self) -> Result<(), RegexError> {
		let Some(ch) = self.next_char() else {
			return Err(RegexError);
		};
		match ch {
			'^' => {
				self.emit(Inst::AnchorStart)?;
			}
			'$' => {
				self.emit(Inst::AnchorEnd)?;
			}
			'.' => {
				self.emit(Inst::Match(Matcher::Any))?;
			}
			'(' => {
				let inner = self.parse_alt_clean()?;
				let _ = inner;
				if self.next_char() != Some(')') {
					return Err(RegexError);
				}
			}
			'[' => {
				let matcher = self.parse_class()?;
				self.emit(Inst::Match(matcher))?;
			}
			'\\' => {
				let matcher = self.parse_escape()?;
				self.emit(Inst::Match(matcher))?;
			}
			'*' | '+' | '?' => return Err(RegexError),
			other => {
				self.emit(Inst::Match(Matcher::Char(other)))?;
			}
		}
		Ok(())
	}

	fn parse_alt_clean(&mut self) -> Result<usize, RegexError> {
		let start = self.program.len();
		self.parse_concat()?;
		while self.peek() == Some('|') {
			self.next_char();
			let rhs_start = self.program.len();
			self.parse_concat()?;
			let end = self.program.len();
			// Transform [start..rhs_start] [rhs_start..end] into:
			// Split(start+1, rhs_start+2), [A], Jump(end+2), [B]
			self.insert_inst(rhs_start, Inst::Jump(end + 1))?;
			self.insert_inst(start, Inst::Split(start + 1, rhs_start + 2))?;
		}
		Ok(start)
	}

	fn parse_escape(&mut self) -> Result<Matcher, RegexError> {
		let Some(ch) = self.next_char() else {
			return Err(RegexError);
		};
		Ok(match ch {
			'd' => Matcher::Digit(false),
			'D' => Matcher::Digit(true),
			'w' => Matcher::Word(false),
			'W' => Matcher::Word(true),
			's' => Matcher::Space(false),
			'S' => Matcher::Space(true),
			'n' => Matcher::Char('\n'),
			't' => Matcher::Char('\t'),
			'r' => Matcher::Char('\r'),
			other => Matcher::Char(other),
		})
	}

	fn parse_class(&mut self) -> Result<Matcher, RegexError> {
		let negated = if self.peek() == Some('^') {
			self.next_char();
			true
		} else {
			false
		};
		let mut items = Vec::new();
		while let Some(ch) = self.next_char() {
			if ch == ']' && !items.is_empty() {
				return Ok(Matcher::Class { negated, items });
			}
			let atom = if ch == '\\' {
				let Some(esc) = self.next_char() else {
					return Err(RegexError);
				};
				match esc {
					'd' => ClassAtom::Digit,
					'w' => ClassAtom::Word,
					's' => ClassAtom::Space,
					'n' => ClassAtom::Char('\n'),
					't' => ClassAtom::Char('\t'),
					other => ClassAtom::Char(other),
				}
			} else {
				ClassAtom::Char(ch)
			};
			if let ClassAtom::Char(start_ch) = atom
				&& self.peek() == Some('-')
				&& self
					.chars
					.get(self.pos + 1)
					.copied()
					.is_some_and(|c| c != ']')
			{
				self.next_char(); // consume '-'
				let Some(end_ch) = self.next_char() else {
					return Err(RegexError);
				};
				if end_ch < start_ch {
					return Err(RegexError);
				}
				items.push(ClassAtom::Range(start_ch, end_ch));
			} else {
				items.push(atom);
			}
		}
		Err(RegexError)
	}
}
