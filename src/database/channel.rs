use tokio::sync::{mpsc, oneshot};

type CallFunc = Box<dyn FnOnce(&mut rusqlite::Connection) + Send + 'static>;

pub struct Connection {
    tx: mpsc::Sender<CallFunc>,
}

impl Connection {
    pub fn new(mut conn: rusqlite::Connection) -> Self {
        let (req_tx, mut req_rx) = mpsc::channel::<CallFunc>(128);

        std::thread::spawn(move || {
            while let Some(func) = req_rx.blocking_recv() {
                func(&mut conn);
            }
        });

        Self { tx: req_tx }
    }

    pub fn blocking_call<F, R>(&self, func: F) -> Result<R, rusqlite::Error>
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let (resp_tx, resp_rx) = oneshot::channel::<rusqlite::Result<R>>();
        self.tx
            .blocking_send(Box::new(move |conn| {
                let value = func(conn);
                resp_tx.send(value).ok();
            }))
            .expect("channel error");
        resp_rx.blocking_recv().expect("channel error")
    }

    pub async fn call<F, R>(&self, func: F) -> Result<R, rusqlite::Error>
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let (resp_tx, resp_rx) = oneshot::channel::<rusqlite::Result<R>>();
        self.tx
            .send(Box::new(move |conn| {
                let value = func(conn);
                resp_tx.send(value).ok();
            }))
            .await
            .expect("channel error");
        resp_rx.await.expect("channel error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_closure() {
        let s_conn = rusqlite::Connection::open_in_memory().unwrap();
        let a_conn = Connection::new(s_conn);
        let r_2 = a_conn
            .call(|conn| {
                let r = conn.query_row("SELECT 1 + 1", [], |row| row.get::<_, i64>(0))?;
                Ok(r)
            })
            .await
            .unwrap();
        assert_eq!(r_2, 2);
    }

    fn select_add_num(conn: &rusqlite::Connection, num: i64) -> rusqlite::Result<i64> {
        let r = conn.query_row("SELECT 1 + 1", [], |row| row.get::<_, i64>(0))?;
        Ok(num + r)
    }

    #[tokio::test]
    async fn test_func() {
        let s_conn = rusqlite::Connection::open_in_memory().unwrap();
        let a_conn = Connection::new(s_conn);
        let r_3 = a_conn
            .call(|conn| {
                let r = select_add_num(conn, 1)?;
                Ok(r)
            })
            .await
            .unwrap();
        assert_eq!(r_3, 3);
    }

    #[test]
    fn test_sync() {
        let s_conn = rusqlite::Connection::open_in_memory().unwrap();
        let a_conn = Connection::new(s_conn);
        let r_3 = a_conn
            .blocking_call(|conn| {
                let r = select_add_num(conn, 1)?;
                Ok(r)
            })
            .unwrap();
        assert_eq!(r_3, 3);
    }
}
