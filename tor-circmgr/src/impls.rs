//! Implement traits from [`crate::mgr`] for the circuit types we use.

use crate::mgr::{self};
use crate::usage::{SupportedCircUsage, TargetCircUsage};
use crate::{DirInfo, Error, Result};
use async_trait::async_trait;
use rand::{rngs::StdRng, SeedableRng};
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Duration;
use tor_chanmgr::ChanMgr;
use tor_proto::circuit::ClientCirc;
use tor_rtcompat::{Runtime, SleepProviderExt};

impl mgr::AbstractCirc for tor_proto::circuit::ClientCirc {
    type Id = tor_proto::circuit::UniqId;
    fn id(&self) -> Self::Id {
        self.unique_id()
    }
    fn usable(&self) -> bool {
        !self.is_closing()
    }
}

impl mgr::AbstractSpec for SupportedCircUsage {
    type Usage = TargetCircUsage;

    fn supports(&self, usage: &TargetCircUsage) -> bool {
        self.contains(usage)
    }

    fn restrict_mut(&mut self, _usage: &TargetCircUsage) -> Result<()> {
        Ok(())
    }
}

/// The information generated by circuit planning, and used to build a
/// circuit.
pub(crate) struct Plan {
    /// The supported usage that the circuit will have when complete
    final_spec: SupportedCircUsage,
    /// An owned copy of the path to build.
    // TODO: it would be nice if this weren't owned.
    path: crate::path::OwnedPath,
    /// The protocol parameters to use when constructing the circuit.
    params: tor_proto::circuit::CircParameters,
}

/// An object that knows how to build circuits.
pub(crate) struct Builder<R: Runtime> {
    /// The runtime used by this circuit builder.
    runtime: R,
    /// A channel manager that this circuit builder uses to make chanels.
    chanmgr: Arc<ChanMgr<R>>,
}

impl<R: Runtime> Builder<R> {
    /// Construct a new Builder
    pub(crate) fn new(runtime: R, chanmgr: Arc<ChanMgr<R>>) -> Self {
        Builder { runtime, chanmgr }
    }
}

#[async_trait]
impl<R: Runtime> crate::mgr::AbstractCircBuilder for Builder<R> {
    type Circ = ClientCirc;
    type Spec = SupportedCircUsage;
    type Plan = Plan;

    fn plan_circuit(
        &self,
        usage: &TargetCircUsage,
        dir: DirInfo<'_>,
    ) -> Result<(Plan, SupportedCircUsage)> {
        let mut rng = rand::thread_rng();
        let (path, final_spec) = usage.build_path(&mut rng, dir)?;

        let owned_path = (&path).try_into()?;
        let params = dir.circ_params();
        let plan = Plan {
            final_spec: final_spec.clone(),
            path: owned_path,
            params,
        };

        Ok((plan, final_spec))
    }

    async fn build_circuit(&self, plan: Plan) -> Result<(SupportedCircUsage, Arc<ClientCirc>)> {
        let delay = Duration::from_secs(5); // TODO: make this configurable and inferred.

        let chanmgr_copy = Arc::clone(&self.chanmgr);
        let runtime_copy = self.runtime.clone();
        let Plan {
            final_spec,
            path,
            params,
        } = plan;
        let mut rng =
            StdRng::from_rng(rand::thread_rng()).expect("couldn't construct temporary rng");

        let outcome = self
            .runtime
            .timeout(
                delay,
                path.build_circuit(&mut rng, &runtime_copy, &chanmgr_copy, &params),
            )
            .await;

        match outcome {
            Ok(Ok(circ)) => Ok((final_spec, circ)),
            Ok(Err(e)) => Err(e),
            Err(_timeout) => Err(Error::CircTimeout),
        }
    }

    fn launch_parallelism(&self, spec: &TargetCircUsage) -> usize {
        match spec {
            TargetCircUsage::Dir => 3,
            _ => 1,
        }
    }

    fn select_parallelism(&self, spec: &TargetCircUsage) -> usize {
        self.launch_parallelism(spec)
    }
}
