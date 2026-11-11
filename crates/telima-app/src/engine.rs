//! Container, image and volume use cases (D5-D7).
use telima_core::container::{Container, ContainerState, Image, LogLine, PruneReport, Volume};
use telima_core::ports::ContainerEngine;

use crate::AppError;

pub struct EngineService<E> {
    engine: E,
}

impl<E: ContainerEngine> EngineService<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// Running containers first, then by name.
    pub async fn containers(&self) -> Result<Vec<Container>, AppError> {
        let mut list = self.engine.containers().await?;
        list.sort_by(|a, b| {
            let rank = |c: &Container| u8::from(c.state != ContainerState::Running);
            rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name))
        });
        Ok(list)
    }

    pub async fn start_container(&self, id: &str) -> Result<(), AppError> {
        Ok(self.engine.start_container(id).await?)
    }

    pub async fn stop_container(&self, id: &str) -> Result<(), AppError> {
        Ok(self.engine.stop_container(id).await?)
    }

    pub async fn restart_container(&self, id: &str) -> Result<(), AppError> {
        Ok(self.engine.restart_container(id).await?)
    }

    /// Remove a container; a running one is removed only with `force`.
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<(), AppError> {
        if !force {
            let list = self.engine.containers().await?;
            if let Some(c) = list
                .iter()
                .find(|c| c.id == id && c.state == ContainerState::Running)
            {
                return Err(AppError::InUse(c.name.clone()));
            }
        }
        Ok(self.engine.remove_container(id, force).await?)
    }

    pub async fn logs(
        &self,
        id: &str,
        follow: bool,
        sink: &mut (dyn FnMut(LogLine) + Send),
    ) -> Result<(), AppError> {
        Ok(self.engine.logs(id, follow, sink).await?)
    }

    /// Largest first.
    pub async fn images(&self) -> Result<Vec<Image>, AppError> {
        let mut list = self.engine.images().await?;
        list.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
        Ok(list)
    }

    /// Images an image prune would remove, for the confirmation dialog.
    pub async fn image_prune_preview(&self) -> Result<Vec<Image>, AppError> {
        Ok(self
            .images()
            .await?
            .into_iter()
            .filter(Image::is_dangling)
            .collect())
    }

    pub async fn prune_images(&self) -> Result<PruneReport, AppError> {
        Ok(self.engine.prune_images().await?)
    }

    pub async fn volumes(&self) -> Result<Vec<Volume>, AppError> {
        let mut list = self.engine.volumes().await?;
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    /// Volumes a volume prune would remove, for the confirmation dialog.
    pub async fn volume_prune_preview(&self) -> Result<Vec<Volume>, AppError> {
        Ok(self
            .volumes()
            .await?
            .into_iter()
            .filter(Volume::is_unused)
            .collect())
    }

    /// Remove a volume that no container references.
    pub async fn remove_volume(&self, name: &str) -> Result<(), AppError> {
        let list = self.engine.volumes().await?;
        match list.iter().find(|v| v.name == name) {
            Some(v) if v.is_unused() => Ok(self.engine.remove_volume(name).await?),
            Some(_) => Err(AppError::InUse(name.to_owned())),
            None => Err(telima_core::ports::PortError::NotFound(name.to_owned()).into()),
        }
    }

    pub async fn prune_volumes(&self) -> Result<PruneReport, AppError> {
        Ok(self.engine.prune_volumes().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeEngine;
    use futures::executor::block_on;

    fn container(id: &str, name: &str, state: ContainerState) -> Container {
        Container {
            id: id.into(),
            name: name.into(),
            image: "alpine:3".into(),
            state,
            status_text: String::new(),
            ports: Vec::new(),
            created_at: None,
            started_at: None,
        }
    }

    fn image(id: &str, tags: &[&str], size: u64) -> Image {
        Image {
            id: id.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            size_bytes: size,
            containers: None,
        }
    }

    fn volume(name: &str, refs: Option<u64>) -> Volume {
        Volume {
            name: name.into(),
            size_bytes: None,
            ref_count: refs,
        }
    }

    #[test]
    fn d5_running_containers_first_then_by_name() {
        let e = FakeEngine::default();
        *e.containers.lock().unwrap() = vec![
            container("1", "b-exited", ContainerState::Exited),
            container("2", "z-running", ContainerState::Running),
            container("3", "a-exited", ContainerState::Exited),
        ];
        let names: Vec<_> = block_on(EngineService::new(e).containers())
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["z-running", "a-exited", "b-exited"]);
    }

    #[test]
    fn d5_restart_remove_and_logs_reach_engine() {
        let svc = EngineService::new(FakeEngine {
            log_lines: vec!["hello".into()],
            ..Default::default()
        });
        block_on(svc.restart_container("c1")).unwrap();
        block_on(svc.remove_container("c1", true)).unwrap();
        let mut lines = Vec::new();
        block_on(svc.logs("c1", true, &mut |l| lines.push(l.text))).unwrap();
        assert_eq!(
            svc.engine().calls(),
            ["restart c1", "remove c1 force=true", "logs c1 follow=true"]
        );
        assert_eq!(lines, ["hello"]);
    }

    #[test]
    fn d5_running_container_needs_force() {
        let e = FakeEngine::default();
        *e.containers.lock().unwrap() = vec![container("c1", "web", ContainerState::Running)];
        let svc = EngineService::new(e);
        assert_eq!(
            block_on(svc.remove_container("c1", false)),
            Err(AppError::InUse("web".into()))
        );
        assert!(svc.engine().calls().is_empty());
    }

    #[test]
    fn d6_images_largest_first_and_prune_preview_lists_dangling() {
        let e = FakeEngine::default();
        *e.images.lock().unwrap() = vec![
            image("a", &["alpine:3"], 5),
            image("b", &[], 50),
            image("c", &["postgres:17"], 400),
        ];
        let svc = EngineService::new(e);
        let ids: Vec<_> = block_on(svc.images())
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, ["c", "b", "a"]);
        let preview: Vec<_> = block_on(svc.image_prune_preview())
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(preview, ["b"]);
        block_on(svc.prune_images()).unwrap();
        assert_eq!(svc.engine().calls(), ["prune_images"]);
    }

    #[test]
    fn d7_volume_prune_preview_and_in_use_guard() {
        let e = FakeEngine::default();
        *e.volumes.lock().unwrap() = vec![
            volume("db", Some(1)),
            volume("old", Some(0)),
            volume("unknown", None),
        ];
        let svc = EngineService::new(e);
        let preview: Vec<_> = block_on(svc.volume_prune_preview())
            .unwrap()
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(preview, ["old"]);
        assert_eq!(
            block_on(svc.remove_volume("db")),
            Err(AppError::InUse("db".into()))
        );
        assert_eq!(
            block_on(svc.remove_volume("unknown")),
            Err(AppError::InUse("unknown".into()))
        );
        block_on(svc.remove_volume("old")).unwrap();
        block_on(svc.prune_volumes()).unwrap();
        assert_eq!(svc.engine().calls(), ["remove_volume old", "prune_volumes"]);
    }

    #[test]
    fn l6_start_and_stop_reach_the_engine() {
        let svc = EngineService::new(FakeEngine::default());
        block_on(svc.start_container("c1")).unwrap();
        block_on(svc.stop_container("c1")).unwrap();
        assert_eq!(svc.engine().calls(), ["start c1", "stop c1"]);
    }
}
