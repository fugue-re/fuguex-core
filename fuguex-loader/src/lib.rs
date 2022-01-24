pub use either::Either;

use fugue::bytes::Order;
use fugue::db::{Database, DatabaseImporter, Segment};
use fugue::ir::{LanguageDB, Translator};
use fugue::ir::convention::Convention;

use fuguex_state::flat::FlatState;
use fuguex_state::paged::{PagedState, Segment as LoadedSegment};
use fuguex_state::pcode::PCodeState;

#[cfg(feature = "idapro")]
use fugue_idapro as idapro;
#[cfg(feature = "ghidra")]
use fugue_ghidra as ghidra;
#[cfg(feature = "radare")]
use fugue_radare as radare;

use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database import: {0}")]
    Import(#[from] fugue::db::Error),
}

pub trait LoaderMapping<S> {
    fn database(&self) -> Option<Arc<Database>> {
        None
    }

    fn translator(&self) -> Arc<Translator>;
    fn into_state(self) -> S;
}

#[derive(Clone)]
pub struct MappedDatabase<S> {
    database: Arc<Database>,
    state: S,
    translator: Arc<Translator>,
}

impl MappedDatabase<PagedState<u8>> {
    pub fn from_database_with<F>(database: Database, mut segment_filter: F) -> Self
    where F: FnMut(&Segment) -> bool {
        let translator = database.default_translator();
        let space = translator.manager().default_space();
        let mut backing = Vec::default();
        let ivt = database.segments().iter().filter(|(_, v)| segment_filter(v)).map(|(k, v)| {
            let kv = (translator.address(*k.start()).into()..translator.address(1 + *k.end()).into(),
                      LoadedSegment::new(v.name(), backing.len()));

            backing.extend_from_slice(v.bytes());

            let diff = (1 + *k.end() - k.start()) as usize;
            if v.bytes().len() < diff {
                let to_add = diff - v.bytes().len();
                backing.resize_with(backing.len() + to_add, Default::default);
            }

            kv
        }).collect::<Vec<_>>();

        let flat = FlatState::from_vec(space, backing);
        let state = PagedState::from_parts(ivt.into_iter(), flat);

        Self {
            database: Arc::new(database),
            translator: Arc::new(translator),
            state,
        }
    }

    pub fn from_database(database: Database) -> Self {
        Self::from_database_with(database, |_| true)
    }

    pub fn from_path_with<P, F>(path: P, language_db: &LanguageDB, segment_filter: F) -> Result<Self, Error>
    where P: AsRef<Path>,
          F: FnMut(&Segment) -> bool {
        #[allow(unused_mut)]
        let mut dbi = DatabaseImporter::new(path);

        #[cfg(feature = "idapro")]
        dbi.register_backend(idapro::IDA::new().unwrap_or_default());

        #[cfg(feature = "ghidra")]
        dbi.register_backend(ghidra::Ghidra::new().unwrap_or_default());

        #[cfg(feature = "radare")]
        dbi.register_backend(radare::Radare::new().unwrap_or_default());

        let db = dbi.import(language_db)?;

        Ok(Self::from_database_with(db, segment_filter))
    }

    pub fn from_path<P>(path: P, language_db: &LanguageDB) -> Result<Self, Error>
    where P: AsRef<Path> {
        Self::from_path_with(path, language_db, |_| true)
    }

    pub fn pcode_state<O: Order>(self, convention: &Convention) -> MappedDatabase<PCodeState<u8, O>> {
        MappedDatabase {
            state: PCodeState::new(self.state, &self.translator, convention),
            database: self.database,
            translator: self.translator,
        }
    }

    pub fn pcode_state_with<O: Order, C: AsRef<str>>(self, convention: C) -> Either<MappedDatabase<PCodeState<u8, O>>, Self> {
        let convention = convention.as_ref();
        if let Some(convention) = self.translator.compiler_conventions().get(convention) {
            Either::Left(MappedDatabase {
                state: PCodeState::new(self.state, &self.translator, convention),
                database: self.database,
                translator: self.translator,
            })
        } else {
            Either::Right(self)
        }
    }
}

impl<S> LoaderMapping<S> for MappedDatabase<S> {
    fn database(&self) -> Option<Arc<Database>> {
        Some(self.database.clone())
    }

    fn translator(&self) -> Arc<Translator> {
        self.translator.clone()
    }

    fn into_state(self) -> S {
        self.state
    }
}
