// Project: Korvex Video Engine | Profile: Production-Final
// Port: 8081 (Internal Hardware Acceleration Enabled)

use std::sync::Arc;
use tokio::sync::Mutex;
use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use std::process::{Command, Stdio};
use std::fs;
use futures::future::join_all;
use sha2::{Sha256, Digest};

// === COMPILATION EDITION LOGIC ===
const EDITION: &str = "COMMERCIAL (Full)";

// === CONFIG ===
const OUTPUT_DIR: &str = "./video_output";
const TEMP_DIR: &str = "./temp";
static ACTIVE_JOBS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// ================================================================
// SECURITY GATE - HARD-LOCK VALIDATION
// ================================================================
fn get_hwid() -> String {
    machine_uid::get().unwrap_or_else(|_| "UNKNOWN_ID".to_string())
}

fn is_license_valid() -> bool {
    let hwid = get_hwid();
    let tier = "GOLD";
    let secret_salt = "KORVEX_MASTER_SECRET_KEY_2026";
    
    if let Ok(key) = fs::read_to_string("license.key") {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}-{}-{}", hwid, tier, secret_salt));
        let expected = format!("{:x}", hasher.finalize());
        return key.trim() == expected;
    }
    false
}

// === EDITION LIMITS ===
fn get_max_segments() -> usize {
    if is_license_valid() { 100 } else { 1 }
}

fn has_watermark() -> bool {
    !is_license_valid()
}

// ================================================================
// DATA MODELS
// ================================================================
#[derive(Deserialize, Debug, Clone)]
pub struct VideoJob {
    pub job_id: Option<String>,
    pub segments: Vec<VideoSegment>,
    pub bgm_path: Option<String>,
    pub output_name: String,
    pub resolution: String,
    pub fps: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VideoSegment {
    pub segment_id: String,
    pub text: String,
    pub image_path: String,
    pub duration_seconds: f32,
}

#[derive(Serialize, Debug, Clone)]
pub struct JobStatus {
    pub job_id: String,
    pub status: String,
    pub progress: f32,
    pub segments_done: usize,
    pub total_segments: usize,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub edition: String,
}

#[derive(Debug, Clone)]
struct JobState {
    pub job: VideoJob,
    pub status: JobStatus,
}

// ================================================================
// VIDEO ENGINE CORE
// ================================================================
struct VideoEngine {
    jobs: Arc<Mutex<HashMap<String, Arc<Mutex<JobState>>>>>,
}

impl VideoEngine {
    fn new() -> Self {
        let _ = fs::create_dir_all(OUTPUT_DIR);
        let _ = fs::create_dir_all(TEMP_DIR);
        Self { jobs: Arc::new(Mutex::new(HashMap::new())) }
    }

    async fn create_job(&self, mut job: VideoJob) -> Result<JobStatus, String> {
        if job.segments.len() > get_max_segments() {
            return Err(format!("Limit Exceeded: {} version is restricted to {} segments.", EDITION, get_max_segments()));
        }
        
        let job_id = job.job_id.take().unwrap_or_else(|| Uuid::new_v4().to_string());
        let status = JobStatus {
            job_id: job_id.clone(),
            status: "PROCESSING".to_string(),
            progress: 0.0,
            segments_done: 0,
            total_segments: job.segments.len(),
            output_path: None,
            error: None,
            edition: EDITION.to_string(),
        };

        let job_state = Arc::new(Mutex::new(JobState { job, status: status.clone() }));
        self.jobs.lock().await.insert(job_id.clone(), job_state.clone());
        ACTIVE_JOBS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let engine_clone = self.clone();
        tokio::spawn(async move { engine_clone.process_full_video(job_id, job_state).await; });
        Ok(status)
    }

    async fn process_full_video(self, job_id: String, state: Arc<Mutex<JobState>>) {
        let segments = { state.lock().await.job.segments.clone() };
        let mut render_tasks = Vec::new();

        for segment in segments {
            let engine_ref = self.clone();
            let job_id_ref = job_id.clone();
            let state_ref = state.clone();
            render_tasks.push(tokio::spawn(async move {
                engine_ref.render_segment(&job_id_ref, &segment, &state_ref).await
            }));
        }

        let results = join_all(render_tasks).await;
        let mut is_success = true;

        for res in results {
            if let Ok(Err(e)) = res {
                let mut s = state.lock().await;
                s.status.error = Some(e);
                s.status.status = "FAILED".into();
                is_success = false;
            }
        }

        if is_success {
            if let Err(e) = self.concat_and_finalize(&job_id, &state).await {
                let mut s = state.lock().await;
                s.status.error = Some(e);
                s.status.status = "FAILED".into();
            }
        }

        let _ = fs::remove_dir_all(format!("{}/{}", TEMP_DIR, job_id));
        ACTIVE_JOBS.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    async fn render_segment(&self, job_id: &str, seg: &VideoSegment, state: &Arc<Mutex<JobState>>) -> Result<(), String> {
        let job_temp_dir = format!("{}/{}", TEMP_DIR, job_id);
        let _ = fs::create_dir_all(&job_temp_dir);
        let output_path = format!("{}/seg_{}.mp4", job_temp_dir, seg.segment_id);
        
        let (res, fps) = { let s = state.lock().await; (s.job.resolution.clone(), s.job.fps) };
        let mut filters = format!("scale={},fps={}", res, fps);

        if is_license_valid() {
            let drawtext = format!(",drawtext=text='{}':fontcolor=white:fontsize=40:box=1:boxcolor=black@0.6:boxborderw=10:x=(w-text_w)/2:y=h-100",
                seg.text.replace("'", "").replace(":", "\\:"));
            filters.push_str(&drawtext);
        }

        if has_watermark() {
            filters.push_str(",drawtext=text='UNREGISTERED ENGINE':fontcolor=white@0.15:fontsize=45:x=(w-text_w)/2:y=(h-text_h)/2");
        }

        let status = Command::new("ffmpeg").args(&[
            "-y", "-loop", "1", "-i", &seg.image_path,
            "-f", "lavfi", "-i", &format!("anullsrc=r=44100:cl=stereo:d={}", seg.duration_seconds),
            "-t", &seg.duration_seconds.to_string(),
            "-vf", &filters, "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", &output_path
        ]).stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(|e| e.to_string())?;

        if status.success() {
            let mut s = state.lock().await;
            s.status.segments_done += 1;
            s.status.progress = (s.status.segments_done as f32 / s.status.total_segments as f32) * 100.0;
            Ok(())
        } else { Err("FFmpeg Core Error".into()) }
    }

    async fn concat_and_finalize(&self, job_id: &str, state: &Arc<Mutex<JobState>>) -> Result<(), String> {
        let job_temp_dir = format!("{}/{}", TEMP_DIR, job_id);
        let list_path = format!("{}/list.txt", job_temp_dir);
        let segments = { state.lock().await.job.segments.clone() };
        
        let mut list_content = String::new();
        for seg in segments { list_content.push_str(&format!("file 'seg_{}.mp4'\n", seg.segment_id)); }
        fs::write(&list_path, list_content).map_err(|e| e.to_string())?;

        let output_name = { state.lock().await.job.output_name.clone() };
        let final_path = format!("{}/{}.mp4", OUTPUT_DIR, output_name);

        let status = Command::new("ffmpeg").args(&["-y", "-f", "concat", "-safe", "0", "-i", &list_path, "-c", "copy", &final_path])
            .stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(|e| e.to_string())?;

        if status.success() {
            let mut s = state.lock().await;
            s.status.status = "COMPLETED".to_string();
            s.status.output_path = Some(final_path);
            Ok(())
        } else { Err("Final Render Assembly Failed".into()) }
    }
}

impl Clone for VideoEngine { fn clone(&self) -> Self { Self { jobs: self.jobs.clone() } } }

// === API HANDLERS ===
async fn create_job_handler(job: web::Json<VideoJob>, data: web::Data<VideoEngine>) -> impl Responder {
    match data.create_job(job.into_inner()).await {
        Ok(s) => HttpResponse::Ok().json(s),
        Err(e) => HttpResponse::BadRequest().body(e),
    }
}

async fn list_jobs_handler(data: web::Data<VideoEngine>) -> impl Responder {
    let jobs = data.jobs.lock().await;
    let mut statuses = Vec::new();
    for j in jobs.values() { statuses.push(j.lock().await.status.clone()); }
    HttpResponse::Ok().json(statuses)
}

async fn status_handler() -> impl Responder {
    let license_status = if is_license_valid() { "VALID" } else { "INVALID/UNREGISTERED" };
    format!("----------------------------------------------\nKORVEX VIDEO FACTORY v1.1\nEdition: {}\nLicense: {}\nHWID: {}\nActive Jobs: {}\n----------------------------------------------", 
        EDITION, license_status, get_hwid(), ACTIVE_JOBS.load(std::sync::atomic::Ordering::Relaxed))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let engine = web::Data::new(VideoEngine::new());
    
    println!("?? KORVEX VIDEO ENGINE STARTING...");
    println!("?? Port: 8081");
    println!("?? Security: Hard-Lock SHA256 Enabled");

    HttpServer::new(move || {
        App::new().app_data(engine.clone())
            .route("/api/v1/job", web::post().to(create_job_handler))
            .route("/api/v1/jobs", web::get().to(list_jobs_handler))
            .route("/api/v1/status", web::get().to(status_handler))
    })
    .bind("0.0.0.0:8081")?
    .run().await
}
