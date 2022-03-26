use songbird::input::{reader::MediaSource, Codec, Container, Input, Reader};
use std::io::{Read, Seek, SeekFrom};

fn mp3_to_songbird_input<R: Read + Seek + Send + 'static>(source: R) -> Input {
    // format::Context::Input
    // ctx.decoder().
    // let encoding = duct::cmd!(
    //     "ffmpeg",
    //     "-i",
    //     "-",
    //     "-f",
    //     "s16le",
    //     "-ac",
    //     "2",
    //     "-ar",
    //     "48000",
    //     "-acodec",
    //     "pcm_s16le",
    //     "-",
    // )
    // .stdin_file(source)
    // .read
    // let encoder = Command::new("ffmpeg")
    //     .stdin(reader)
    //     .stdout(Stdio::piped())
    //     .args([
    // "-i",
    // "-",
    // "-f",
    // "s16le",
    // "-ac",
    // "2",
    // "-ar",
    // "48000",
    // "-acodec",
    // "pcm_s16le",
    // "-",
    //     ])
    //     .spawn();
    // let decoder = codec::decoder::find_by_name("mp3");
    // println!("{:?}", decoder);
    // let input = songbird::input::Input::new;
    let decoder = rodio::Decoder::new_mp3(source).unwrap();
    let source = RodioMediaSource { decoder };
    let reader = Reader::Extension(Box::new(source));
    Input::new(true, reader, Codec::Pcm, Container::Raw, None)
}

struct RodioMediaSource<R>
where
    R: Read + Seek + Send,
{
    decoder: rodio::Decoder<R>,
}

impl<R> MediaSource for RodioMediaSource<R>
where
    R: Read + Seek + Send,
{
    fn is_seekable(&self) -> bool {
        true
    }

    fn len(&self) -> Option<u64> {
        None
    }
}

impl<R> Seek for RodioMediaSource<R>
where
    R: Read + Seek + Send,
{
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported",
        ))
    }
}

impl<R> Read for RodioMediaSource<R>
where
    R: Read + Seek + Send,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let sample_count = buf.len() / 2;
        let mut count = 0;
        for _ in 0..sample_count {
            let sample = self.decoder.next();
            match sample {
                None => {
                    break;
                }
                Some(sample) => {
                    for byte in sample.to_ne_bytes().into_iter() {
                        buf[count] = byte;
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }
}
