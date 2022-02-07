use async_trait::async_trait;

pub trait Executable {
    fn exec(&mut self);
}

#[async_trait]
pub trait AsyncExecutable {
    async fn exec(&mut self) -> anyhow::Result<()>;

    #[inline]
    fn get_priority(&self) -> usize {
        0
    }
}
