#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(not(windows))]
compile_error!("Morrison Capture Studio requires Windows for WASAPI capture.");

mod audio;
use eframe::egui;
use std::{path::{Path,PathBuf}, process::{Child,Command,Stdio}, sync::{Arc,atomic::{AtomicBool,Ordering}}, thread::JoinHandle, time::{SystemTime,UNIX_EPOCH}};

#[derive(PartialEq, Clone, Copy)]
enum AudioMode { Off, Speakers, Microphone, Both }
impl AudioMode { fn label(self)-> &'static str {match self {Self::Off=>"No audio",Self::Speakers=>"Speakers only",Self::Microphone=>"Microphone only",Self::Both=>"Speakers + microphone"}} }
#[derive(PartialEq, Clone, Copy)]
enum Tab { Record, Edit, Files }
struct Recording { stop:Arc<AtomicBool>, threads:Vec<JoinHandle<Result<(),String>>>, video: Option<Child>, folder:PathBuf, base:String, mode:AudioMode, has_video:bool }
struct Studio {
 tab:Tab, mode:AudioMode, speakers:Vec<(String,String)>, microphones:Vec<(String,String)>, speaker_idx:usize,microphone_idx:usize,
 screen:bool, frame_rate:u32, region:[i32;4], mouse:bool, recording:Option<Recording>,
 chosen:String, export:String, out_format:usize, trim_start:f32,trim_end:f32, gain_db:f32,speed:f32,fade_in:f32,fade_out:f32,
 normalize:bool, reverse:bool,mono:bool, crop:[i32;4],resize:[u32;2],mute_video:bool,overlay_text:String,
 status:String, output_dir:PathBuf, assets:Vec<PathBuf>, ffmpeg_path:PathBuf, debug_log:String
}
fn data_dir()->PathBuf {std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("Videos").join("Morrison Capture Studio")}
#[cfg(feature = "bundle-ffmpeg")]
fn install_embedded_ffmpeg(target:&Path) {
    if !target.exists() {
        let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/ffmpeg.exe"));
        let _ = std::fs::write(target, bytes);
    }
}
fn ffmpeg()->PathBuf { let exe=std::env::current_exe().unwrap_or_default();let adjacent=exe.parent().unwrap_or(Path::new(".")).join("ffmpeg.exe");#[cfg(feature="bundle-ffmpeg")]
    install_embedded_ffmpeg(&adjacent);
    if adjacent.exists(){adjacent}else{PathBuf::from("ffmpeg")}}
fn quoted(s:&str)->String{ s.replace('"', "'") }
impl Default for Studio {
 fn default()->Self {let output_dir=data_dir();let _=std::fs::create_dir_all(&output_dir);let mut s=Self{
 tab:Tab::Record,mode:AudioMode::Speakers,speakers:vec![],microphones:vec![],speaker_idx:0,microphone_idx:0,
 screen:false,frame_rate:30,region:[0,0,0,0],mouse:true,recording:None,chosen:String::new(),export:String::new(),out_format:0,
 trim_start:0.,trim_end:0.,gain_db:0.,speed:1.,fade_in:0.,fade_out:0.,normalize:false,reverse:false,mono:false,crop:[0,0,0,0],resize:[0,0],mute_video:false,overlay_text:String::new(),status:"Ready. Speakers only is the default; microphone capture is OFF.".into(),output_dir,assets:vec![],ffmpeg_path:ffmpeg(),debug_log:String::new()};s.refresh_devices();s.refresh_files();s }
}
impl Studio {
 fn refresh_devices(&mut self){self.speakers=audio::list_devices(false).unwrap_or_default();self.microphones=audio::list_devices(true).unwrap_or_default();self.speaker_idx=self.speaker_idx.min(self.speakers.len().saturating_sub(1));self.microphone_idx=self.microphone_idx.min(self.microphones.len().saturating_sub(1));}
 fn refresh_files(&mut self){self.assets=std::fs::read_dir(&self.output_dir).into_iter().flatten().filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().and_then(|s|s.to_str()).map(|x|matches!(x.to_ascii_lowercase().as_str(),"wav"|"mp3"|"flac"|"aac"|"m4a"|"ogg"|"mp4"|"mkv"|"webm"|"gif")).unwrap_or(false)).collect();self.assets.sort();self.assets.reverse();}
 fn start_record(&mut self){if self.recording.is_some(){return;}if self.mode==AudioMode::Off && !self.screen {self.status="Choose an audio source or enable screen capture.".into();return;}
 let t=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();let base=format!("Capture_{t}");let stop=Arc::new(AtomicBool::new(false));let mut threads=vec![];
 if matches!(self.mode,AudioMode::Speakers|AudioMode::Both){let id=self.speakers.get(self.speaker_idx).map(|x|x.1.clone());threads.push(audio::spawn_capture(false,id,self.output_dir.join(format!("{base}_speakers.wav")),stop.clone()));}
 if matches!(self.mode,AudioMode::Microphone|AudioMode::Both){let id=self.microphones.get(self.microphone_idx).map(|x|x.1.clone());threads.push(audio::spawn_capture(true,id,self.output_dir.join(format!("{base}_microphone.wav")),stop.clone()));}
 let mut video=None;
 if self.screen {let mut cmd=Command::new(&self.ffmpeg_path);cmd.args(["-hide_banner","-loglevel","error","-y","-f","gdigrab","-framerate",&self.frame_rate.to_string(),"-draw_mouse",if self.mouse{"1"}else{"0"}]);
 let [x,y,w,h]=self.region;if w>0&&h>0 {cmd.args(["-offset_x",&x.to_string(),"-offset_y",&y.to_string(),"-video_size",&format!("{w}x{h}")]);}cmd.args(["-i","desktop","-c:v","libx264","-preset","ultrafast","-pix_fmt","yuv420p","-crf","20"]);cmd.arg(self.output_dir.join(format!("{base}_screen.mp4")));cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());match cmd.spawn(){Ok(c)=>video=Some(c),Err(e)=>{stop.store(true,Ordering::Relaxed);for h in threads{let _=h.join();}self.status=format!("Screen capture could not start. Check ffmpeg.exe: {e}");return;}}}
 self.recording=Some(Recording{stop,threads,video,folder:self.output_dir.clone(),base,mode:self.mode,has_video:self.screen});self.status="RECORDING. Use Stop to finalize WAV and MP4 files.".into();}
 fn stop_record(&mut self){if let Some(mut rec)=self.recording.take(){rec.stop.store(true,Ordering::Relaxed);let mut failures=vec![];for t in rec.threads.drain(..){match t.join(){Ok(Err(e))=>failures.push(e),Err(_)=>failures.push("Audio worker panicked".into()),_=>{}}}
 if let Some(mut v)=rec.video.take(){if let Some(stdin)=v.stdin.as_mut(){use std::io::Write;let _=stdin.write_all(b"q\n");}match v.wait(){Ok(st) if !st.success()=>failures.push(format!("Screen encoder exit status: {st}")),Err(e)=>failures.push(e.to_string()),_=>{}}}
 self.refresh_files();let prefix=rec.folder.join(&rec.base);let speaker=prefix.with_file_name(format!("{}_speakers.wav",rec.base));let mic=prefix.with_file_name(format!("{}_microphone.wav",rec.base));let screen=prefix.with_file_name(format!("{}_screen.mp4",rec.base));
 // Create a single usable video with explicit audio routing. Original isolated audio stems are preserved.
 if rec.has_video && rec.mode!=AudioMode::Off && screen.exists(){let out=prefix.with_file_name(format!("{}_video_with_audio.mp4",rec.base));let mut args=vec!["-y".to_string(),"-i".into(),screen.to_string_lossy().to_string()];
 if matches!(rec.mode,AudioMode::Speakers|AudioMode::Both){args.extend(["-i".into(),speaker.to_string_lossy().to_string()]);}
 if matches!(rec.mode,AudioMode::Microphone|AudioMode::Both){args.extend(["-i".into(),mic.to_string_lossy().to_string()]);}
 let filter= if rec.mode==AudioMode::Both {"[1:a][2:a]amix=inputs=2:duration=longest:normalize=0[a]"}else{"[1:a]anull[a]"};args.extend(["-filter_complex".into(),filter.into(),"-map".into(),"0:v:0".into(),"-map".into(),"[a]".into(),"-c:v".into(),"copy".into(),"-c:a".into(),"aac".into(),"-shortest".into(),out.to_string_lossy().to_string()]);if let Err(e)=self.run_ffmpeg(args){failures.push(format!("Audio/video mux: {e}"));} }
 self.refresh_files();self.status=if failures.is_empty(){format!("Saved recording to {}",rec.folder.display())}else{format!("Recording ended with errors: {}",failures.join("; "))};}}
 fn run_ffmpeg(&mut self,args:Vec<String>)->Result<(),String>{let output=Command::new(&self.ffmpeg_path).args(&args).output().map_err(|e|format!("Cannot launch FFmpeg: {e}. Put ffmpeg.exe beside the app."))?;self.debug_log=format!("ffmpeg {}\n{}",args.iter().map(|v|quoted(v)).collect::<Vec<_>>().join(" "),String::from_utf8_lossy(&output.stderr));if output.status.success(){Ok(())}else{Err(self.debug_log.chars().rev().take(1100).collect::<String>().chars().rev().collect())}}
 fn do_export(&mut self){let input=PathBuf::from(self.chosen.trim());if !input.is_file(){self.status="Select an existing media file in Files or enter its path.".into();return;}let formats=["mp3","wav","flac","m4a","ogg","mp4","mkv","webm","gif"];
 let ext=formats[self.out_format];let output=if self.export.trim().is_empty(){self.output_dir.join(format!("{}_edited.{ext}",input.file_stem().unwrap_or_default().to_string_lossy()))}else{PathBuf::from(self.export.trim())};if input==output{self.status="Choose a different output name so the original remains safe.".into();return;}
 let mut args=vec!["-hide_banner".into(),"-y".into()];if self.trim_start>0. {args.extend(["-ss".into(),format!("{:.3}",self.trim_start)]);}args.extend(["-i".into(),input.to_string_lossy().to_string()]);if self.trim_end>self.trim_start && self.trim_end>0. {args.extend(["-t".into(),format!("{:.3}",self.trim_end-self.trim_start)]);}
 let is_video=["mp4","mkv","webm","gif"].contains(&ext);let mut af=vec![];
 if self.gain_db.abs()>0.05 {af.push(format!("volume={}dB",self.gain_db));}if self.normalize{af.push("loudnorm=I=-16:TP=-1.5:LRA=11".into());}if self.mono{af.push("aformat=channel_layouts=mono".into());}if self.reverse{af.push("areverse".into());}
 if (self.speed-1.).abs()>0.005{let mut remaining=self.speed as f64;while remaining>2.0{af.push("atempo=2.0".into());remaining/=2.;}while remaining<0.5{af.push("atempo=0.5".into());remaining/=0.5;}af.push(format!("atempo={remaining:.6}"));}
 if self.fade_in>0. {af.push(format!("afade=t=in:st=0:d={:.2}",self.fade_in));}if self.fade_out>0. && self.trim_end>self.trim_start {let length=(self.trim_end-self.trim_start)/self.speed.max(0.25);af.push(format!("afade=t=out:st={:.2}:d={:.2}",(length-self.fade_out).max(0.),self.fade_out));}
 if is_video {let mut vf=vec![];let [x,y,w,h]=self.crop;if w>0&&h>0{vf.push(format!("crop={w}:{h}:{x}:{y}"));}if self.resize[0]>0&&self.resize[1]>0{vf.push(format!("scale={}:{}",self.resize[0],self.resize[1]));}if (self.speed-1.).abs()>0.005 {vf.push(format!("setpts=PTS/{:.5}",self.speed));}
 if !vf.is_empty(){args.extend(["-vf".into(),vf.join(",")]);}
 if ext=="gif" {args.extend(["-an".into(),"-loop".into(),"0".into()]);}else{if self.mute_video{args.push("-an".into());}else if !af.is_empty(){args.extend(["-af".into(),af.join(",")]);}if ext=="webm" {args.extend(["-c:v".into(),"libvpx-vp9".into(),"-c:a".into(),"libopus".into()]);}else{args.extend(["-c:v".into(),"libx264".into(),"-preset".into(),"fast".into(),"-crf".into(),"20".into(),"-pix_fmt".into(),"yuv420p".into(),"-c:a".into(),"aac".into(),"-b:a".into(),"192k".into()]);}}}
 else {args.push("-vn".into());if !af.is_empty(){args.extend(["-af".into(),af.join(",")]);}match ext{"mp3"=>args.extend(["-c:a".into(),"libmp3lame".into(),"-b:a".into(),"256k".into()]),"wav"=>args.extend(["-c:a".into(),"pcm_s24le".into()]),"flac"=>args.extend(["-c:a".into(),"flac".into()]),"m4a"=>args.extend(["-c:a".into(),"aac".into(),"-b:a".into(),"256k".into()]),"ogg"=>args.extend(["-c:a".into(),"libvorbis".into(),"-q:a".into(),"6".into()]),_=>{}}}
 args.push(output.to_string_lossy().to_string());self.status=match self.run_ffmpeg(args){Ok(())=>format!("Exported: {}",output.display()),Err(e)=>format!("Export failed: {e}")};self.refresh_files();}
 fn recording_controls(&mut self,ui:&mut egui::Ui){ui.heading("Record audio or your screen");ui.label("Select exactly the sources you want. No microphone audio is recorded in Speakers only mode.");
 ui.horizontal_wrapped(|ui|for m in [AudioMode::Off,AudioMode::Speakers,AudioMode::Microphone,AudioMode::Both]{ui.selectable_value(&mut self.mode,m,m.label());});
 if matches!(self.mode,AudioMode::Speakers|AudioMode::Both){ui.horizontal(|ui|{ui.label("Speaker / output device:");egui::ComboBox::from_id_salt("speaker").selected_text(self.speakers.get(self.speaker_idx).map(|x|x.0.as_str()).unwrap_or("Default speaker")).show_ui(ui,|ui|for (i,(name,_)) in self.speakers.iter().enumerate(){ui.selectable_value(&mut self.speaker_idx,i,name);});});}
 if matches!(self.mode,AudioMode::Microphone|AudioMode::Both){ui.horizontal(|ui|{ui.label("Microphone / input device:");egui::ComboBox::from_id_salt("microphone").selected_text(self.microphones.get(self.microphone_idx).map(|x|x.0.as_str()).unwrap_or("Default microphone")).show_ui(ui,|ui|for (i,(name,_)) in self.microphones.iter().enumerate(){ui.selectable_value(&mut self.microphone_idx,i,name);});});}
 if ui.button("Refresh audio devices").clicked(){self.refresh_devices();}ui.separator();ui.checkbox(&mut self.screen,"Record desktop video");if self.screen{ui.horizontal(|ui|{ui.label("FPS");ui.add(egui::Slider::new(&mut self.frame_rate,10..=60));ui.checkbox(&mut self.mouse,"Include cursor");});ui.label("Screen capture region (X, Y, Width, Height). Width/Height = 0 captures full desktop.");ui.horizontal(|ui|for (i,label) in ["X","Y","Width","Height"].iter().enumerate(){ui.label(*label);ui.add(egui::DragValue::new(&mut self.region[i]).speed(2));});}
 ui.separator();ui.label(format!("Recording folder: {}",self.output_dir.display()));ui.horizontal(|ui|{if self.recording.is_none(){if ui.add(egui::Button::new("⏺ START RECORDING").fill(egui::Color32::from_rgb(136,45,45))).clicked(){self.start_record();}}else if ui.button("■ STOP AND SAVE").clicked(){self.stop_record();}});
 ui.small("Speaker and microphone audio are recorded to separate WAV stems. If both are selected, video export also includes a mixed track.");}
 fn editor_controls(&mut self,ui:&mut egui::Ui){ui.heading("Edit and export");ui.label("Non-destructive edits. Original files are never overwritten.");ui.horizontal(|ui|{ui.label("Open file");ui.text_edit_singleline(&mut self.chosen);});ui.horizontal(|ui|{ui.label("Trim from (s)");ui.add(egui::DragValue::new(&mut self.trim_start).range(0.0..=360000.0).speed(0.25));ui.label("to (s, 0 = end)");ui.add(egui::DragValue::new(&mut self.trim_end).range(0.0..=360000.0).speed(0.25));});
 ui.horizontal(|ui|{ui.label("Volume (dB)");ui.add(egui::Slider::new(&mut self.gain_db,-30.0..=24.0));ui.label("Speed");ui.add(egui::Slider::new(&mut self.speed,0.25..=4.0));});
 ui.horizontal(|ui|{ui.label("Fade in (s)");ui.add(egui::DragValue::new(&mut self.fade_in).range(0.0..=3600.0));ui.label("Fade out (s)");ui.add(egui::DragValue::new(&mut self.fade_out).range(0.0..=3600.0));});
 ui.horizontal(|ui|{ui.checkbox(&mut self.normalize,"Normalize loudness");ui.checkbox(&mut self.reverse,"Reverse audio");ui.checkbox(&mut self.mono,"Convert to mono");});ui.separator();ui.label("Video tools (applied only to video exports):");ui.horizontal(|ui|{ui.checkbox(&mut self.mute_video,"Remove audio");ui.label("Crop X Y W H:");for a in &mut self.crop{ui.add(egui::DragValue::new(a).speed(2));}});ui.horizontal(|ui|{ui.label("Resize W H (0 = unchanged):");for a in &mut self.resize{ui.add(egui::DragValue::new(a).speed(2));}});
 ui.separator();let formats=["mp3","wav","flac","m4a","ogg","mp4","mkv","webm","gif"];ui.horizontal(|ui|{ui.label("Save format:");egui::ComboBox::from_id_salt("format").selected_text(formats[self.out_format]).show_ui(ui,|ui|for (i,v) in formats.iter().enumerate(){ui.selectable_value(&mut self.out_format,i,*v);});});ui.horizontal(|ui|{ui.label("Save as (empty = automatic):");ui.text_edit_singleline(&mut self.export);});if ui.button("EXPORT EDITED FILE").clicked(){self.do_export();}
 ui.small("FFmpeg is required for media exports. WAV float recordings display a clickable waveform; video preview and multitrack editing are not implemented.");}
 fn file_controls(&mut self,ui:&mut egui::Ui){ui.heading("Recordings and exports");if ui.button("Refresh folder").clicked(){self.refresh_files();}egui::ScrollArea::vertical().show(ui,|ui|{for p in &self.assets{let s=p.to_string_lossy().to_string();ui.horizontal(|ui|{if ui.button("Edit").clicked(){self.chosen=s.clone();self.tab=Tab::Edit;}ui.label(p.file_name().unwrap_or_default().to_string_lossy());});}});}
}
impl eframe::App for Studio {fn update(&mut self,ctx:&egui::Context,_frame:&mut eframe::Frame){egui::CentralPanel::default().show(ctx,|ui|{ui.heading("MORRISON CAPTURE STUDIO  •  RUST EDITION");ui.label("Windows audio loopback + microphone + screen recording and media exporting");ui.separator();ui.horizontal(|ui|{ui.selectable_value(&mut self.tab,Tab::Record,"⏺ RECORD");ui.selectable_value(&mut self.tab,Tab::Edit,"✂ EDIT / EXPORT");ui.selectable_value(&mut self.tab,Tab::Files,"▤ RECORDINGS");});ui.separator();egui::ScrollArea::vertical().max_height(ui.available_height()-115.).show(ui,|ui|match self.tab{Tab::Record=>self.recording_controls(ui),Tab::Edit=>self.editor_controls(ui),Tab::Files=>self.file_controls(ui)});ui.separator();ui.label(egui::RichText::new(&self.status).color(egui::Color32::LIGHT_GREEN));ui.small(format!("FFmpeg: {}",self.ffmpeg_path.display()));});}}
fn main()->eframe::Result<()> {let opts=eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([970.0,690.0]).with_min_inner_size([790.0,570.0]).with_title("Morrison Capture Studio"),..Default::default()};eframe::run_native("Morrison Capture Studio",opts,Box::new(|_|Ok(Box::new(Studio::default()))))}
