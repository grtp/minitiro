use reqwest;
use urlencoding::encode;
use serde_json;
use serde_json::Value;

pub async fn get_speakers() -> Option<Vec<Vec<String>>> {
    let result = reqwest::get("http://127.0.0.1:50021/speakers").await;
    match result {
        Ok(res) => {
            let res : Value = serde_json::from_str(&res.text().await.unwrap()).unwrap();
            let res = res.as_array().unwrap();
            let mut all :Vec<Vec<String>> = Vec::new();
            for el in res.iter() {
                let styles = el.get("styles").unwrap().as_array().unwrap();
                for s in styles.iter() {
                    let mut part_of: Vec<String> = Vec::new();
                    let id = &*s.get("id").unwrap();
                    part_of.push(serde_json::to_string(&id).unwrap());
                    let name = &*el.get("name").unwrap();
                    part_of.push(serde_json::to_string(&name).unwrap());
                    let tp = &*s.get("name").unwrap();
                    part_of.push(serde_json::to_string(&tp).unwrap());
                    let version = &*el.get("version").unwrap();
                    part_of.push(serde_json::to_string(&version).unwrap());
                    all.push(part_of);
                }
            }
            return Some(all);
        },
        _ => return None,
    };
}

pub async fn create_wav(text: &str, speaker: u32) -> Option<Vec<u8>> {
    // create audio json data.
    let client = reqwest::Client::new();
    let mut url = "http://127.0.0.1:50021/audio_query".to_string();
    let mut query = "?".to_string();
    let mut q_speaker = "speaker=".to_string();
    q_speaker.push_str(&speaker.to_string());
    query.push_str(&q_speaker);
    let text = encode(text).into_owned();
    let mut q_text = "&text=".to_string();
    q_text.push_str(&text);
    query.push_str(&q_text);
    url.push_str(&query);
    let json = get_audio_json(url, &client);
    let Some(json) = json.await else { println!("[Err] json have not data"); return None; };
    
    // create audio wav data with audio json data.
    let mut url = "http://127.0.0.1:50021/synthesis".to_string();
    let mut q_speaker = "?speaker=".to_string();
    q_speaker.push_str(&speaker.to_string());
    url.push_str(&q_speaker);
    let wav = get_audio_wav(url, json, &client).await;
    wav
}

async fn get_audio_json(url: String, client: &reqwest::Client) -> Option<String> {
    let res = client.post(url)
        .send()
        .await
        .unwrap();
    if let Ok(value) = res.text().await {
            let json: Value = serde_json::from_str(&value).unwrap();
            return Some(json.to_string());
    } else { println!("[Err] json: failed to parse response."); return None; }
}

async fn get_audio_wav(url: String, json: String, client: &reqwest::Client) -> Option<Vec<u8>> {
    let json = serde_json::from_str(&json).unwrap();
    let res = client.post(url)
        .header("Content-Type", "application/json")
        .json::<Value>(&json)
        .send()
        .await
        .unwrap();
    if let Ok(value) = res.bytes().await {
        return Some(value.to_vec());
    } else {
        println!("[Err] wav: failed to parse response.");
        return None;
    }
}