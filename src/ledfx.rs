pub fn playpause(baseurl: &str, state: bool) -> Result<(), ureq::Error> {
    let url = format!("{baseurl}/api/virtuals");
    let result: serde_json::Value = ureq::get(url.as_str()).call()?.into_json()?;
    // println!("RESULT: {:?}", result);
    if let Some(serde_json::Value::Bool(ispaused)) = result.get("paused") {
        // println!("We are paused? {}", &ispaused);
        if ispaused != &state {
            // println!("Swapping states!");
            let _result: serde_json::Value = ureq::put(url.as_str()).call()?.into_json()?;
        }
    };
    Ok(())
}

pub fn is_playing(baseurl: &str)->Result<bool, ureq::Error>{
    let url = format!("{baseurl}/api/virtuals");
    let result: serde_json::Value = ureq::get(url.as_str()).call()?.into_json()?;
    // println!("RESULT: {:?}", result);
    let current_state = if let Some(serde_json::Value::Bool(ispaused)) = result.get("paused") {
        true
    }else{
        false
    };
    Ok(current_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread::sleep, time::Duration};

    // #[test]
    // fn test_playpause() {
    //     playpause("http://localhost:8888", true);
    //     sleep(Duration::from_secs(3));
    //     playpause("http://localhost:8888", false);
    // }
}
