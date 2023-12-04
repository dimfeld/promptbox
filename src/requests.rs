use serde::Serialize;

pub fn request_with_retry(
    req: ureq::Request,
    body: impl Serialize,
) -> Result<ureq::Response, ureq::Error> {
    const MAX_TRIES: u32 = 4;
    let mut try_num = 0;
    let delay = 1000;
    loop {
        let response = req.clone().send_json(&body);
        match response {
            Ok(res) => return Ok(res),
            Err(ureq::Error::Status(code, response)) => {
                if code != 429 || try_num > MAX_TRIES {
                    return Err(ureq::Error::Status(code, response));
                }

                // This is potentially retryable. We don't do anything smart right now, just a
                // random exponential backoff.

                let perturb = fastrand::i32(-100..100);
                let this_delay = 2i32.pow(try_num) * delay + perturb;

                eprintln!("Rate limited... waiting {this_delay}ms to retry");
                std::thread::sleep(std::time::Duration::from_millis(this_delay as u64));
                try_num += 1;
            }
            e @ Err(_) => return e,
        }
    }
}
