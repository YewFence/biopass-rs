use std::time::{Duration, Instant};
use v4l::io::mmap::Stream as MmapStream;
use v4l::io::traits::CaptureStream;

pub(super) fn next_frame_before(
    stream: &mut MmapStream<'_>,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match stream.next() {
            Ok((buffer, _)) => return Ok(buffer.to_vec()),
            Err(error) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    continue;
                }
            }
            Err(error) => return Err(format!("Failed to read V4L2 frame: {error}")),
        }
    }
}
