use anyhow::{Result, anyhow, ensure};

use super::{State, Test, Transaction};

const MAGIC: u32 = 0xD33D_BAE0;
const INITIAL: u32 = 1;
const FINAL: u32 = 2;
const TRANSACTIONS: u32 = 3;
const OPCODE: u32 = 4;
const TRANSACTION_WORDS: usize = 6;

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<Reader<'a>> {
        ensure!(len <= self.0.len(), "truncated ARM7TDMI record");
        let (head, tail) = self.0.split_at(len);
        self.0 = tail;
        Ok(Reader(head))
    }

    fn word(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.0.try_into().unwrap()))
    }

    fn words<const N: usize>(&mut self) -> Result<[u32; N]> {
        let mut words = [0; N];
        for word in &mut words {
            *word = self.word()?;
        }
        Ok(words)
    }

    fn record(&mut self) -> Result<Reader<'a>> {
        let size = self.word()? as usize;
        ensure!(size >= 4, "invalid ARM7TDMI record size {size}");
        self.take(size - 4)
    }

    fn section(&mut self, tag: u32) -> Result<Reader<'a>> {
        let mut section = self.record()?;
        let actual = section.word()?;
        ensure!(actual == tag, "expected section {tag}, got {actual}");
        Ok(section)
    }

    fn finish(self) -> Result<()> {
        ensure!(self.0.is_empty(), "unexpected trailing ARM7TDMI data");
        Ok(())
    }

    fn state(&mut self, tag: u32) -> Result<State> {
        let mut section = self.section(tag)?;

        let state = State {
            registers: section.words()?,
            fiq: section.words()?,
            svc: section.words()?,
            abt: section.words()?,
            irq: section.words()?,
            und: section.words()?,
            cpsr: section.word()?,
            spsr: section.words()?,
            pipeline: section.words()?,
        };
        let _access = section.word()?;

        section.finish()?;
        Ok(state)
    }

    fn test(&mut self) -> Result<Test> {
        let mut record = self.record()?;
        let initial = record.state(INITIAL)?;
        let expected = record.state(FINAL)?;

        let mut section = record.section(TRANSACTIONS)?;
        let count = section.word()? as usize;
        ensure!(
            count <= section.0.len() / (TRANSACTION_WORDS * 4),
            "invalid transaction count {count}"
        );
        let mut transactions = Vec::with_capacity(count);
        for _ in 0..count {
            transactions.push(Transaction {
                kind: section.word()?,
                size: section.word()?,
                addr: section.word()?,
                data: section.word()?,
            });
            let _cycle = section.word()?;
            let _access = section.word()?;
        }
        section.finish()?;

        let mut section = record.section(OPCODE)?;
        let opcode = section.word()?;
        let base_addr = section.word()?;
        section.finish()?;

        record.finish()?;
        Ok(Test {
            name: None,
            initial,
            expected,
            transactions,
            opcode,
            base_addr,
        })
    }
}

pub(super) struct Tests<'a> {
    reader: Reader<'a>,
    remaining: u32,
}

impl<'a> Tests<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self> {
        let mut reader = Reader(bytes);
        ensure!(reader.word()? == MAGIC, "invalid ARM7TDMI file magic");
        let remaining = reader.word()?;
        Ok(Self { reader, remaining })
    }
}

impl Iterator for Tests<'_> {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            if self.reader.0.is_empty() {
                return None;
            }
            self.reader.0 = &[];
            return Some(Err(anyhow!("unexpected trailing ARM7TDMI tests")));
        }

        self.remaining -= 1;
        Some(self.reader.test())
    }
}
