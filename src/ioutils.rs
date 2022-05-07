use std::{
    collections::{hash_map::Iter as HashMapIter, HashMap},
    io::{self, Cursor, Read},
    sync::mpsc::{self, Receiver, Sender},
};

struct Registry<M> {
    auto_increment: u64,
    members: HashMap<u64, M>,
}

impl<M> Default for Registry<M> {
    fn default() -> Self {
        Self {
            auto_increment: 0,
            members: HashMap::new(),
        }
    }
}

impl<M> Registry<M> {
    fn register(&mut self, member: M) -> u64 {
        let id = self.auto_increment;
        self.members.insert(id, member);
        self.auto_increment += 1;
        id
    }

    fn unregister(&mut self, id: u64) {
        self.members.remove(&id);
    }

    fn iter(&self) -> HashMapIter<'_, u64, M> {
        self.members.iter()
    }
}

pub struct Tapped {
    tap_id: u64,
    unregister: Sender<u64>,
    current_slice: Option<Cursor<Vec<u8>>>,
    receiver: Option<Receiver<Vec<u8>>>,
}

impl Tapped {
    fn read_next_slice(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self
            .receiver
            .as_mut()
            .and_then(|receiver| receiver.recv().ok())
        {
            Some(slice) => {
                let mut slice = Cursor::new(slice);
                let result = slice.read(buf);
                self.current_slice = Some(slice);
                result
            }
            None => {
                self.receiver = None;
                Ok(0)
            }
        }
    }

    fn read_current_slice(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let result = self.current_slice.as_mut().unwrap().read(buf);
        if let Ok(0) = result {
            return self.read_next_slice(buf);
        }
        result
    }
}

impl Drop for Tapped {
    fn drop(&mut self) {
        let _result = self.unregister.send(self.tap_id);
    }
}

impl Read for Tapped {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.current_slice.is_none() {
            self.read_next_slice(buf)
        } else {
            self.read_current_slice(buf)
        }
    }
}

pub struct TappableReader<R>
where
    R: Read,
{
    source: R,
    taps: Registry<Sender<Vec<u8>>>,
    shutdown: Receiver<u64>,
    shutdown_sender: Sender<u64>,
}

impl<R: Read> TappableReader<R> {
    pub fn new(source: R) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            source,
            taps: Registry::default(),
            shutdown: receiver,
            shutdown_sender: sender,
        }
    }
    pub fn tap(&mut self) -> Tapped {
        let (sender, receiver) = mpsc::channel();
        let tap_id = self.taps.register(sender);
        Tapped {
            current_slice: None,
            tap_id,
            receiver: Some(receiver),
            unregister: self.shutdown_sender.clone(),
        }
    }
}

impl<R: Read> Read for TappableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.source.read(buf) {
            Ok(n) => {
                self.send_to_taps(&buf[..n]);
                Ok(n)
            }
            Err(err) => {
                self.close_taps();
                Err(err)
            }
        }
    }
}

impl<R: Read> TappableReader<R> {
    fn reconcile_taps(&mut self) {
        while let Ok(to_unregister) = self.shutdown.try_recv() {
            self.taps.unregister(to_unregister);
        }
    }
    fn send_to_taps(&mut self, data: &[u8]) {
        self.reconcile_taps();
        let slice = Vec::from(data);
        for (_, sender) in self.taps.iter() {
            sender.send(slice.clone()).unwrap();
        }
    }

    fn close_taps(&mut self) {
        for sender in self.taps.iter() {
            drop(sender);
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use std::io::{Cursor, Read};

    use super::TappableReader;

    #[test]
    fn test_normal_read() {
        let reader = Cursor::new(b"hello world").reader();
        let mut reader = TappableReader::new(reader);
        let mut output = vec![];
        reader.read_to_end(&mut output).unwrap();
        assert_eq!(Vec::from(b"hello world"), output);
    }

    #[test]
    fn test_tapping_read() {
        let reader = Cursor::new(b"hello world").reader();
        let mut reader = TappableReader::new(reader);
        let mut tapped = reader.tap();
        let mut output = vec![];
        reader.read_to_end(&mut output).unwrap();
        let mut tapped_output = vec![];
        tapped.read_to_end(&mut tapped_output).unwrap();
        assert_eq!(Vec::from(b"hello world"), tapped_output);
    }
}
