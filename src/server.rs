use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use crate::processor::Processor;
use crate::scanner::Scanner;
use anyhow::Result;
use dashmap::DashMap;
use indicatif::{MultiProgress, ProgressDrawTarget};
use rand::Rng;
use threadpool::ThreadPool;

use crate::fileinfo::{FileInfo, FileSource};
use crate::params::Params;

pub struct Server {
    filequeue: Arc<Mutex<Vec<FileInfo>>>,
    sw_duplicate_set: Arc<DashMap<u64, Vec<FileInfo>>>,
    pub hw_duplicate_set: Arc<DashMap<u128, Vec<FileInfo>>>,
    threadpool: ThreadPool,
    app_args: Arc<Params>,
    pub max_file_path_len: Arc<AtomicU64>,
}

impl Server {
    pub fn new(opts: Params) -> Self {
        Self {
            filequeue: Arc::new(Mutex::new(Vec::new())),
            sw_duplicate_set: Arc::new(DashMap::new()),
            hw_duplicate_set: Arc::new(DashMap::new()),
            threadpool: ThreadPool::new(4),
            app_args: Arc::new(opts),
            max_file_path_len: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(&self) -> Result<()> {
        // In comparison mode, scan both staging and target directories first
        if self.app_args.comparison_mode {
            let staging_dir = self.app_args.get_staging_directory()?;
            let target_dir = self.app_args.get_target_directory()?;

            let scanner = Scanner::build(&self.app_args)?;
            
            let mut staging_files = scanner.scan_with_source(staging_dir, FileSource::Staging)?;
            let mut target_files = scanner.scan_with_source(target_dir, FileSource::Target)?;

            // Combine all files and populate the queue
            staging_files.append(&mut target_files);
            {
                let mut queue = self.filequeue.lock().unwrap();
                *queue = staging_files;
            }
        }
        let progbarbox = Arc::new(MultiProgress::new());
        let mut rng = rand::rng();
        let seed: i64 = rng.random();

        if !self.app_args.progress {
            progbarbox.set_draw_target(ProgressDrawTarget::hidden());
        }

        let (app_args_sc, app_args_sw, app_args_hw) = (
            Arc::clone(&self.app_args),
            Arc::clone(&self.app_args),
            Arc::clone(&self.app_args),
        );
        let (file_queue_sc, file_queue_pr) = (
            Arc::clone(&self.filequeue),
            Arc::clone(&self.filequeue),
        );
        
        let scanner_finished = Arc::new(AtomicBool::new(self.app_args.comparison_mode));
        let sw_sort_finished = Arc::new(AtomicBool::new(false));
        let (sfin_sc, sfin_pr) = (
            Arc::clone(&scanner_finished),
            Arc::clone(&scanner_finished),
        );
        let (swfin_pr_sw, swfin_pr_hw) = (
            Arc::clone(&sw_sort_finished),
            Arc::clone(&sw_sort_finished),
        );
        let (store_sw, store_sw2, store_hw) = (
            Arc::clone(&self.sw_duplicate_set),
            Arc::clone(&self.sw_duplicate_set),
            Arc::clone(&self.hw_duplicate_set),
        );
        let max_file_path_len = Arc::clone(&self.max_file_path_len);
        let (prog_sc, prog_sw, prog_hw) = (
            Arc::clone(&progbarbox),
            Arc::clone(&progbarbox),
            Arc::clone(&progbarbox),
        );

        if !self.app_args.comparison_mode {
            self.threadpool.execute(move || {
                Scanner::new(app_args_sc)
                    .expect("unable to initialize scanner.")
                    .scan(file_queue_sc, prog_sc)
                    .expect("scanner failed.");

                sfin_sc.store(true, std::sync::atomic::Ordering::Relaxed);
            });
        }

        self.threadpool.execute(move || {
            Processor::sizewise(
                app_args_sw,
                sfin_pr,
                store_sw,
                file_queue_pr,
                prog_sw,
            )
            .expect("sizewise scanner failed.");

            swfin_pr_sw.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        self.threadpool.execute(move || {
            Processor::hashwise(
                app_args_hw,
                store_sw2,
                store_hw,
                prog_hw,
                max_file_path_len,
                seed,
                swfin_pr_hw,
            )
            .expect("hashwise scanner failed.");
        });

        progbarbox.clear()?;

        self.threadpool.join();

        Ok(())
    }
}
