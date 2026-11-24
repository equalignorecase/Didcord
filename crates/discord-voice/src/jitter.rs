//! Adaptive 40-160ms jitter buffer with bounded 320ms reorder window for 2.5-120ms Opus packets.
const SLOTS: usize = 16;
const MIN_WAIT: u8 = 2;
const MAX_WAIT: u8 = 8;
const STABLE_RECOVER_PACKETS: u16 = 150;

pub(crate) enum Popped {
	Packet(Vec<u8>),
	Fec(Vec<u8>),
	Conceal,
}

pub(crate) struct Jitter {
	packets: Vec<(u16, Vec<u8>)>,
	next: Option<u16>,
	wait: u8,
	target_wait: u8,
	missing: u8,
	stable_streak: u16,
	losses: u64,
}

impl Default for Jitter {
	fn default() -> Self {
		Self {
			packets: Vec::new(),
			next: None,
			wait: 0,
			target_wait: MIN_WAIT,
			missing: 0,
			stable_streak: 0,
			losses: 0,
		}
	}
}

impl Jitter {
	pub fn clear(&mut self) {
		*self = Self {
			target_wait: self.target_wait,
			..Self::default()
		};
	}

	pub fn take_losses(&mut self) -> u64 {
		std::mem::take(&mut self.losses)
	}

	fn note_congestion(&mut self) {
		self.stable_streak = 0;
		self.losses += 1;
		self.target_wait = self.target_wait.saturating_add(1).min(MAX_WAIT);
	}

	pub fn push(&mut self, sequence: u16, opus: Vec<u8>) {
		if opus.len() > 1275 {
			return;
		}
		if self.next.is_none() {
			self.next = Some(sequence);
			self.wait = self.target_wait;
		}
		let distance = sequence.wrapping_sub(self.next.unwrap());
		if distance >= 32768 {
			// Recently late packets signal network jitter: grow buffer slightly for future bursts.
			if distance >= u16::MAX - 31 {
				self.note_congestion();
			}
			return;
		}
		if distance >= SLOTS as u16 {
			if distance < 64 {
				// Advance just enough to keep recently arrived future packets across a jitter spike.
				let new_next = sequence.wrapping_sub(SLOTS as u16 - 1);
				self.packets
					.retain(|(id, _)| id.wrapping_sub(new_next) < SLOTS as u16);
				self.next = Some(new_next);
				self.missing = 0;
				self.note_congestion();
			} else {
				self.packets.clear();
				self.next = Some(sequence);
				self.wait = self.target_wait;
				self.missing = 0;
			}
		}
		if self.packets.len() < SLOTS && !self.packets.iter().any(|(id, _)| *id == sequence) {
			self.packets.push((sequence, opus));
		}
	}

	#[cfg(test)]
	pub fn pop(&mut self) -> Option<Vec<u8>> {
		match self.pop_frame()? {
			Popped::Packet(opus) => Some(opus),
			Popped::Fec(_) | Popped::Conceal => Some(Vec::new()),
		}
	}

	/// Returns the next in-order packet, next-packet FEC payload when available, or concealment.
	pub fn pop_frame(&mut self) -> Option<Popped> {
		if self.wait > 0 {
			// Short packets can fill the fixed window before startup delay finishes. Start before
			// the next arrival would repeatedly reset a full window.
			if self.packets.len() < SLOTS {
				self.wait -= 1;
				return None;
			}
			self.wait = 0;
		}
		let next = self.next?;
		self.next = Some(next.wrapping_add(1));
		if let Some(index) = self.packets.iter().position(|(id, _)| *id == next) {
			self.missing = 0;
			self.stable_streak = self.stable_streak.saturating_add(1);
			if self.stable_streak >= STABLE_RECOVER_PACKETS {
				self.stable_streak = 0;
				self.target_wait = self.target_wait.saturating_sub(1).max(MIN_WAIT);
			}
			return Some(Popped::Packet(self.packets.swap_remove(index).1));
		}
		self.missing += 1;
		if self.missing == 1 {
			self.note_congestion();
		} else {
			self.losses += 1;
		}
		if self.missing > 3 {
			// Stop concealing the gap, but retain speech that already arrived after it.
			if let Some((index, (sequence, _))) = self
				.packets
				.iter()
				.enumerate()
				.min_by_key(|(_, (sequence, _))| sequence.wrapping_sub(next))
			{
				self.next = Some(sequence.wrapping_add(1));
				self.missing = 0;
				return Some(Popped::Packet(self.packets.swap_remove(index).1));
			}
			self.clear();
			return None;
		}
		let lookahead = next.wrapping_add(1);
		if let Some((_, fec_opus)) = self.packets.iter().find(|(id, _)| *id == lookahead) {
			return Some(Popped::Fec(fec_opus.clone()));
		}
		Some(Popped::Conceal)
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn burst_loss_preserves_buffered_recovery_and_resets_on_far_jump() {
		for first in [0u16, u16::MAX - 2] {
			let mut jitter = Jitter::default();
			jitter.push(first, vec![0]);
			for offset in [7, 5, 6] {
				jitter.push(first.wrapping_add(offset), vec![offset as u8]);
			}
			assert!(jitter.pop().is_none());
			assert!(jitter.pop().is_none());
			assert_eq!(jitter.pop(), Some(vec![0]));
			for _ in 0..3 {
				assert_eq!(jitter.pop(), Some(vec![]));
			}
			for offset in [5, 6, 7] {
				assert_eq!(jitter.pop(), Some(vec![offset]));
			}
			for _ in 0..2 {
				assert_eq!(jitter.pop(), Some(vec![]));
			}
			jitter.push(first.wrapping_add(100), vec![100]);
			assert_eq!(jitter.missing, 0);
			for _ in 0..jitter.target_wait {
				assert!(jitter.pop().is_none());
			}
			assert_eq!(jitter.pop(), Some(vec![100]));
		}
	}
	#[test]
	fn bounded_reordering_loss_duplicates_and_wrap() {
		let mut jitter = Jitter::default();
		jitter.push(u16::MAX, vec![1]);
		jitter.push(1, vec![3]);
		jitter.push(0, vec![2]);
		jitter.push(0, vec![9]);
		assert!(jitter.pop().is_none());
		assert!(jitter.pop().is_none());
		assert_eq!(jitter.pop(), Some(vec![1]));
		assert_eq!(jitter.pop(), Some(vec![2]));
		assert_eq!(jitter.pop(), Some(vec![3]));
		for _ in 0..3 {
			assert_eq!(jitter.pop(), Some(vec![]));
		}
		assert!(jitter.pop().is_none());
		assert!(jitter.pop().is_none());
		for sequence in 0..1000 {
			jitter.push(sequence, vec![1; 1275]);
			assert!(jitter.packets.len() <= SLOTS);
		}
		jitter.push(1000, vec![0; 1276]);
		assert!(jitter.packets.len() <= SLOTS);
	}

	#[test]
	fn adaptive_wait_grows_on_loss_and_recovers_after_clean_streak() {
		let mut jitter = Jitter::default();
		assert_eq!(jitter.target_wait, MIN_WAIT);
		jitter.push(0, vec![1]);
		jitter.push(2, vec![3]);
		// Two initial startup waits (40ms), packet 0, then packet 1 missing -> target_wait grows.
		assert!(jitter.pop().is_none());
		assert!(jitter.pop().is_none());
		assert_eq!(jitter.pop(), Some(vec![1]));
		assert_eq!(jitter.pop(), Some(vec![]));
		assert_eq!(jitter.target_wait, MIN_WAIT + 1);

		// Sustained clean packets recover target_wait back toward MIN_WAIT.
		for seq in 3..=(3 + STABLE_RECOVER_PACKETS) {
			jitter.push(seq, vec![7]);
			assert!(jitter.pop().is_some());
		}
		assert_eq!(jitter.target_wait, MIN_WAIT);
	}
}
