use std::sync::Arc;

use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::Mutex,
};

pub struct Drawer {
    file: Arc<Mutex<File>>,
}

impl Drawer {
    pub async fn new(game_name: String) -> Self {
        let file_name = format!("{}.farmio", game_name);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_name.clone())
            .await
            .unwrap_or_else(|_| panic!("Unable to Open file: {}", file_name));
        let file = Arc::new(Mutex::new(file));
        Self { file }
    }

    pub async fn write(&mut self, msg: String) {
        let file = self.file.clone();
        //tokio::spawn(async move {
        let mut file = file.lock().await;
        if let Err(err) = file.write_all(msg.as_bytes()).await {
            eprintln!("Unable to write to farmio file: `{}`", err);
        }
        //});
    }

    pub async fn clear(&mut self) {
        let file = self.file.clone();
        //tokio::spawn(async move {
        let mut file = file.lock().await;
        if let Err(err) = file.seek(std::io::SeekFrom::Start(0)).await {
            eprintln!("Unable to jump to the begining of the file: `{}`", err);
        }
        //});
    }
}
