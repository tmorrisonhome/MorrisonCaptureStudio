#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(not(windows))]
compile_error!("Morrison Capture Studio requires Windows for WASAPI capture.");

mod audio;
#[repr(C)]
#[derive(Default,Clone,Copy)]
struct WinPoint {x:i32,y:i32}
#[link(name="user32")]
extern "system" { fn GetAsyncKeyState(vkey:i32)->i16; fn GetCursorPos(point:*mut WinPoint)->i32; }
fn monitor_clicks(stop:Arc<AtomicBool>, clicks:Arc<Mutex<Vec<(f32,i32,i32)>>>) -> JoinHandle<()> {
 std::thread::spawn(move||{
  let start=std::time::Instant::now();let mut previous=false;
  while !stop.load(Ordering::Relaxed){
   let down=unsafe{GetAsyncKeyState(0x01)} < 0;
   if down&&!previous {let mut pt=WinPoint::default();if unsafe{GetCursorPos(&mut pt)}!=0{if let Ok(mut log)=clicks.lock(){log.push((start.elapsed().as_secs_f32(),pt.x,pt.y));}}}
   previous=down;std::thread::sleep(std::time::Duration::from_millis(15));
  }
 })
}

use eframe::egui;
use serde::{Serialize,Deserialize};
use std::os::windows::process::CommandExt;
trait NoWindow { fn creation_flags_no_window(&mut self)->&mut Self; }
impl NoWindow for Command { fn creation_flags_no_window(&mut self)->&mut Self {self.creation_flags(0x08000000)} }
use std::{path::{Path,PathBuf}, process::{Child,Command,Stdio}, sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}, thread::JoinHandle, time::{SystemTime,UNIX_EPOCH}};

#[derive(PartialEq, Clone, Copy)]
enum AudioMode { Off, Speakers, Microphone, Both }
impl AudioMode { fn label(self)-> &'static str {match self {Self::Off=>"No audio",Self::Speakers=>"Speakers only",Self::Microphone=>"Microphone only",Self::Both=>"Speakers + microphone"}} }
#[derive(Clone,PartialEq,Serialize,Deserialize)]
struct EditState { trim_start:f32, trim_end:f32, gain_db:f32, speed:f32, fade_in:f32, fade_out:f32, normalize:bool, reverse:bool, mono:bool, bass_db:f32, treble_db:f32, noise_reduce:bool, compressor:bool, brightness:f32, contrast:f32, saturation:f32, grayscale:bool, flip_h:bool, rotation:i32, crop:[i32;4], resize:[u32;2], mute_video:bool }
#[derive(PartialEq, Clone, Copy)]
enum Tab { Record, Edit, Files }
#[derive(Clone,Serialize,Deserialize)]
struct AudioTrack { path: String, start: f32, gain_db: f32, muted: bool }
#[derive(Clone,Serialize,Deserialize)]
struct VideoClip { path:String, start:f32, end:f32 }
#[derive(Serialize,Deserialize)]
struct Project { media: String, export: String, edit: EditState, tracks: Vec<AudioTrack>, #[serde(default)] clips: Vec<VideoClip> }
struct Recording { stop:Arc<AtomicBool>, threads:Vec<JoinHandle<Result<(),String>>>, video: Option<Child>, folder:PathBuf, base:String, mode:AudioMode, has_video:bool, clicks:Arc<Mutex<Vec<(f32,i32,i32)>>>, click_thread:Option<JoinHandle<()>>, highlight:bool, origin:[i32;2] }
struct Studio {
 tab:Tab, mode:AudioMode, speakers:Vec<(String,String)>, microphones:Vec<(String,String)>, speaker_idx:usize,microphone_idx:usize,
 screen:bool, frame_rate:u32, region:[i32;4], mouse:bool, recording:Option<Recording>,
 chosen:String, export:String, out_format:usize, trim_start:f32,trim_end:f32, gain_db:f32,speed:f32,fade_in:f32,fade_out:f32,
 normalize:bool, reverse:bool,mono:bool, bass_db:f32, treble_db:f32, noise_reduce:bool, compressor:bool, brightness:f32, contrast:f32, saturation:f32, grayscale:bool, flip_h:bool, rotation:i32, crop:[i32;4],resize:[u32;2],mute_video:bool,
 status:String, output_dir:PathBuf, assets:Vec<PathBuf>, ffmpeg_path:PathBuf, debug_log:String, record_started:Option<std::time::Instant>, preview:Option<egui::TextureHandle>, waveform:Vec<f32>, media_duration:f32, highlight_clicks:bool, undo:Vec<EditState>, redo:Vec<EditState>, video_size:[i32;2], playhead:f32, is_playing:bool,  preview_job:Option<std::sync::mpsc::Receiver<(f32,Vec<u8>)>>, preview_at:f32, crop_drag_start:Option<egui::Pos2>, tracks:Vec<AudioTrack>, clips:Vec<VideoClip>, project_path:String, playback_started:Option<std::time::Instant>, playback_from:f32, audio_player:Option<Child>
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
 trim_start:0.,trim_end:0.,gain_db:0.,speed:1.,fade_in:0.,fade_out:0.,normalize:false,reverse:false,mono:false,bass_db:0.,treble_db:0.,noise_reduce:false,compressor:false,brightness:0.,contrast:1.,saturation:1.,grayscale:false,flip_h:false,rotation:0,crop:[0,0,0,0],resize:[0,0],mute_video:false,status:"Ready. Speakers only is the default; microphone capture is OFF.".into(),output_dir,assets:vec![],ffmpeg_path:ffmpeg(),debug_log:String::new(),record_started:None,preview:None,waveform:vec![],media_duration:0.0,highlight_clicks:false,undo:vec![],redo:vec![],video_size:[0,0],playhead:0.,is_playing:false,preview_job:None,preview_at:-1.,crop_drag_start:None,tracks:vec![],clips:vec![],project_path:String::new(),playback_started:None,playback_from:0.,audio_player:None};s.refresh_devices();s.refresh_files();s }
}
impl Studio {

 fn pick_file(save:bool,filter:&str)->Option<String>{
 let class=if save{"SaveFileDialog"}else{"OpenFileDialog"};
 let script=format!("Add-Type -AssemblyName System.Windows.Forms; $d=New-Object System.Windows.Forms.{class}; $d.Filter='{filter}'; if($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ [Console]::Write($d.FileName) }}");
 let output=Command::new("powershell.exe").args(["-NoProfile","-STA","-Command",&script]).creation_flags_no_window().output().ok()?;
 let result=String::from_utf8_lossy(&output.stdout).trim().to_owned();if result.is_empty(){None}else{Some(result)}
 }
 fn choose_folder()->Option<PathBuf>{let script="Add-Type -AssemblyName System.Windows.Forms; $d=New-Object System.Windows.Forms.FolderBrowserDialog; if($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {[Console]::Write($d.SelectedPath)}";
 let output=Command::new("powershell.exe").args(["-NoProfile","-STA","-Command",script]).creation_flags_no_window().output().ok()?;let txt=String::from_utf8_lossy(&output.stdout).trim().to_owned();if txt.is_empty(){None}else{Some(PathBuf::from(txt))}}
 fn project_default_path(&self)->PathBuf {self.output_dir.join("MorrisonCaptureStudio_project.json")}
 fn save_project(&mut self){let path=if self.project_path.trim().is_empty(){self.project_default_path()}else{PathBuf::from(self.project_path.trim())};let project=Project{media:self.chosen.clone(),export:self.export.clone(),edit:self.snapshot(),tracks:self.tracks.clone(),clips:self.clips.clone()};match serde_json::to_vec_pretty(&project).map_err(|e|e.to_string()).and_then(|b|std::fs::write(&path,b).map_err(|e|e.to_string())){Ok(())=>{self.project_path=path.to_string_lossy().into_owned();self.status=format!("Project saved: {}",path.display());},Err(e)=>self.status=format!("Project save failed: {e}")}}
 fn load_project(&mut self,ctx:&egui::Context){let path=if self.project_path.trim().is_empty(){self.project_default_path()}else{PathBuf::from(self.project_path.trim())};let data=std::fs::read(&path).map_err(|e|e.to_string()).and_then(|b|serde_json::from_slice::<Project>(&b).map_err(|e|e.to_string()));match data{Ok(project)=>{self.stop_playback();self.tracks=project.tracks;self.clips=project.clips;self.export=project.export;let edit=project.edit;if Path::new(&project.media).exists(){self.selected_media(ctx,PathBuf::from(project.media));}self.restore(edit);self.status=format!("Project loaded: {}",path.display());},Err(e)=>self.status=format!("Project load failed: {e}")}}
 fn stop_playback(&mut self){self.is_playing=false;self.playback_started=None;if let Some(mut child)=self.audio_player.take(){let _=child.kill();let _=child.wait();}}
 fn start_playback(&mut self){if self.chosen.is_empty(){self.status="Select a media file first".into();return;}self.stop_playback();self.playback_from=self.playhead;self.playback_started=Some(std::time::Instant::now());self.is_playing=true;let exe=std::env::current_exe().unwrap_or_default();let player=exe.parent().unwrap_or(Path::new(".")).join("ffplay.exe");
 #[cfg(feature="bundle-ffmpeg")]
 if !player.exists(){let _=std::fs::write(&player,include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/vendor/ffplay.exe")));}let chosen=self.chosen.clone();if player.exists(){let mut cmd=Command::new(player);cmd.args(["-nodisp","-autoexit","-loglevel","error","-ss",&format!("{:.3}",self.playhead),"-i",&chosen]);cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());self.audio_player=cmd.spawn().ok();}self.status="Playing preview. Use Pause / Stop to stop.".into();}
 fn update_playback(&mut self,ctx:&egui::Context){if !self.is_playing{return;}let elapsed=self.playback_started.map(|t|t.elapsed().as_secs_f32()).unwrap_or(0.);let now=self.playback_from+elapsed;if self.media_duration>0.&&now>=self.media_duration{self.playhead=self.media_duration;self.stop_playback();return;}self.playhead=now;if self.video_size[0]>0&&self.preview_job.is_none()&&self.playhead-self.preview_at>=0.16 {self.seek_preview(ctx,self.playhead);}ctx.request_repaint_after(std::time::Duration::from_millis(80));}

 fn add_video_clip(&mut self){let p=PathBuf::from(&self.chosen);if !p.is_file(){self.status="Select a video file".into();return;}if !["mp4","webm","mkv"].contains(&p.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase().as_str()){self.status="Select an MP4, MKV or WebM file".into();return;}self.clips.push(VideoClip{path:self.chosen.clone(),start:self.trim_start.max(0.),end:if self.trim_end>self.trim_start{self.trim_end}else{self.media_duration.max(0.)}});self.status="Video clip added to timeline".into();}
 fn render_video_timeline(&mut self){
 if self.clips.is_empty(){self.status="Add a video clip to the timeline".into();return;}
 let clips=self.clips.iter().filter(|c|Path::new(&c.path).is_file()&&c.end>c.start).cloned().collect::<Vec<_>>();if clips.len()!=self.clips.len(){self.status="A video clip is missing or has an invalid trim range".into();return;}
 let output=if self.export.trim().is_empty(){self.output_dir.join("edited_video_timeline.mp4")}else{PathBuf::from(self.export.trim())};
 if output.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase()!="mp4"{self.status="Video timeline output must end in .mp4".into();return;}
 if clips.iter().any(|c|Path::new(&c.path)==output){self.status="Output cannot overwrite source video".into();return;}
 let mut args=vec!["-hide_banner".into(),"-y".into()];
 for c in &clips{args.extend(["-ss".into(),format!("{:.3}",c.start),"-t".into(),format!("{:.3}",c.end-c.start),"-i".into(),c.path.clone()]);}
 let mut filters=Vec::new();let mut inputs=String::new();
 for (i,c) in clips.iter().enumerate(){
   filters.push(format!("[{i}:v]fps=30,scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,setsar=1,format=yuv420p,setpts=PTS-STARTPTS[v{i}]"));
   let probe=Command::new(&self.ffmpeg_path).args(["-hide_banner","-i",&c.path]).output();
   let has_audio=probe.map(|o|String::from_utf8_lossy(&o.stderr).lines().any(|line|line.contains("Stream #")&&line.contains("Audio:"))).unwrap_or(false);
   if has_audio{filters.push(format!("[{i}:a]aresample=48000,aformat=channel_layouts=stereo,atrim=duration={:.3},asetpts=PTS-STARTPTS[a{i}]",c.end-c.start));}
   else{filters.push(format!("anullsrc=r=48000:cl=stereo,atrim=duration={:.3},asetpts=PTS-STARTPTS[a{i}]",c.end-c.start));}
   inputs.push_str(&format!("[v{i}][a{i}]"));
 }
 filters.push(format!("{inputs}concat=n={}:v=1:a=1[vout][aout]",clips.len()));
 args.extend(["-filter_complex".into(),filters.join(";"),"-map".into(),"[vout]".into(),"-map".into(),"[aout]".into(),"-c:v".into(),"libx264".into(),"-preset".into(),"medium".into(),"-crf".into(),"20".into(),"-pix_fmt".into(),"yuv420p".into(),"-c:a".into(),"aac".into(),"-b:a".into(),"192k".into(),output.to_string_lossy().into_owned()]);
 self.status=match self.run_ffmpeg(args){Ok(())=>format!("Video timeline exported: {}",output.display()),Err(e)=>format!("Video timeline export failed: {e}")};self.refresh_files();
 }
 fn video_timeline_ui(&mut self,ui:&mut egui::Ui){ui.heading("Video Clip Timeline");ui.label("Combine multiple clips in order, with independent start/end points, into an MP4.");if ui.button("+ Add selected video clip").clicked(){self.add_video_clip();}
 let mut move_up=None;let mut move_down=None;let mut remove=None;
 egui::ScrollArea::vertical().max_height(240.).show(ui,|ui|{for (i,c) in self.clips.iter_mut().enumerate(){egui::Frame::group(ui.style()).show(ui,|ui|{ui.horizontal_wrapped(|ui|{ui.strong(format!("Clip {}",i+1));ui.label("In (s)");ui.add(egui::DragValue::new(&mut c.start).range(0.0..=999999.).speed(0.1));ui.label("Out (s)");ui.add(egui::DragValue::new(&mut c.end).range(0.0..=999999.).speed(0.1));if ui.button("↑").clicked(){move_up=Some(i);}if ui.button("↓").clicked(){move_down=Some(i);}if ui.button("Remove").clicked(){remove=Some(i);}});ui.small(&c.path);});}});
 if let Some(i)=remove{self.clips.remove(i);}else if let Some(i)=move_up{if i>0{self.clips.swap(i,i-1);}}else if let Some(i)=move_down{if i+1<self.clips.len(){self.clips.swap(i,i+1);}}
 if ui.button("RENDER VIDEO CLIP TIMELINE").clicked(){self.render_video_timeline();}
 }
 fn audio_mix_export(&mut self){
   let available=self.tracks.iter().filter(|t|!t.muted&&Path::new(&t.path).is_file()).cloned().collect::<Vec<_>>();
   if available.is_empty(){self.status="Add at least one unmuted audio track".into();return;}
   let path=if self.export.trim().is_empty(){self.output_dir.join("multitrack_mix.wav")}else{PathBuf::from(self.export.trim())};
   let ext=path.extension().and_then(|x|x.to_str()).unwrap_or("wav").to_ascii_lowercase();
   if !["wav","mp3","flac","m4a","ogg"].contains(&ext.as_str()){self.status="Multitrack audio export requires WAV, MP3, FLAC, M4A, or OGG output".into();return;}
   if available.iter().any(|x|Path::new(&x.path)==path){self.status="Output cannot overwrite source track".into();return;}
   let mut args=vec!["-hide_banner".into(),"-y".into()];
   for t in &available{args.extend(["-i".into(),t.path.clone()]);}
   let mut filters=Vec::new();let mut labels=String::new();
   for (i,t) in available.iter().enumerate(){let delay=(t.start.max(0.)*1000.) as u64;filters.push(format!("[{i}:a]aformat=sample_rates=48000:channel_layouts=stereo,volume={}dB,adelay={delay}|{delay}[a{i}]",t.gain_db));labels.push_str(&format!("[a{i}]"));}
   filters.push(format!("{labels}amix=inputs={}:duration=longest:dropout_transition=0,alimiter=limit=0.98[mix]",available.len()));
   args.extend(["-filter_complex".into(),filters.join(";"),"-map".into(),"[mix]".into()]);
   match ext.as_str(){"mp3"=>args.extend(["-c:a".into(),"libmp3lame".into(),"-b:a".into(),"256k".into()]),"flac"=>args.extend(["-c:a".into(),"flac".into()]),"m4a"=>args.extend(["-c:a".into(),"aac".into(),"-b:a".into(),"256k".into()]),"ogg"=>args.extend(["-c:a".into(),"libvorbis".into()]),_=>args.extend(["-c:a".into(),"pcm_s24le".into()])};
   args.push(path.to_string_lossy().into_owned());self.status=match self.run_ffmpeg(args){Ok(())=>format!("Multitrack mix exported: {}",path.display()),Err(e)=>format!("Multitrack mix failed: {e}")};self.refresh_files();
 }
 fn multitrack_editor(&mut self,ui:&mut egui::Ui){
   ui.heading("Multitrack Audio Timeline");ui.label("Each track is independent. Adjust its delay, gain or mute before mixing.");
   if ui.button("+ Add selected file as track").clicked(){if Path::new(&self.chosen).is_file(){self.tracks.push(AudioTrack{path:self.chosen.clone(),start:0.,gain_db:0.,muted:false});self.status="Added selected file to timeline".into();}else{self.status="Select a recorded audio file first".into();}}
   let mut remove=None;
   egui::ScrollArea::vertical().max_height(240.).show(ui,|ui|{for (i,track) in self.tracks.iter_mut().enumerate(){
     egui::Frame::group(ui.style()).show(ui,|ui|{ui.horizontal_wrapped(|ui|{ui.label(format!("Track {}",i+1));ui.checkbox(&mut track.muted,"Mute");ui.label("Start (s)");ui.add(egui::DragValue::new(&mut track.start).range(0.0..=36000.0).speed(0.1));ui.label("Gain (dB)");ui.add(egui::Slider::new(&mut track.gain_db,-60.0..=24.0));if ui.button("Remove").clicked(){remove=Some(i);}});ui.small(&track.path);});
   }});
   if let Some(i)=remove{self.tracks.remove(i);}
   if ui.button("EXPORT MULTITRACK AUDIO MIX").clicked(){self.audio_mix_export();}
 }
 fn snapshot(&self)->EditState {EditState{trim_start:self.trim_start,trim_end:self.trim_end,gain_db:self.gain_db,speed:self.speed,fade_in:self.fade_in,fade_out:self.fade_out,normalize:self.normalize,reverse:self.reverse,mono:self.mono,bass_db:self.bass_db,treble_db:self.treble_db,noise_reduce:self.noise_reduce,compressor:self.compressor,brightness:self.brightness,contrast:self.contrast,saturation:self.saturation,grayscale:self.grayscale,flip_h:self.flip_h,rotation:self.rotation,crop:self.crop,resize:self.resize,mute_video:self.mute_video}}
 fn restore(&mut self,s:EditState){self.trim_start=s.trim_start;self.trim_end=s.trim_end;self.gain_db=s.gain_db;self.speed=s.speed;self.fade_in=s.fade_in;self.fade_out=s.fade_out;self.normalize=s.normalize;self.reverse=s.reverse;self.mono=s.mono;self.bass_db=s.bass_db;self.treble_db=s.treble_db;self.noise_reduce=s.noise_reduce;self.compressor=s.compressor;self.brightness=s.brightness;self.contrast=s.contrast;self.saturation=s.saturation;self.grayscale=s.grayscale;self.flip_h=s.flip_h;self.rotation=s.rotation;self.crop=s.crop;self.resize=s.resize;self.mute_video=s.mute_video;}
 fn save_undo(&mut self){if self.undo.len()>75{self.undo.remove(0);}self.undo.push(self.snapshot());self.redo.clear();}
 fn undo_edit(&mut self){if let Some(s)=self.undo.pop(){self.redo.push(self.snapshot());self.restore(s);self.status="Undo applied".into();}}
 fn redo_edit(&mut self){if let Some(s)=self.redo.pop(){self.undo.push(self.snapshot());self.restore(s);self.status="Redo applied".into();}}
 fn reset_edits(&mut self){self.save_undo();self.restore(EditState{trim_start:0.,trim_end:0.,gain_db:0.,speed:1.,fade_in:0.,fade_out:0.,normalize:false,reverse:false,mono:false,bass_db:0.,treble_db:0.,noise_reduce:false,compressor:false,brightness:0.,contrast:1.,saturation:1.,grayscale:false,flip_h:false,rotation:0,crop:[0;4],resize:[0;2],mute_video:false});}
 fn split_at_playhead(&mut self){
    let src=PathBuf::from(self.chosen.trim());if !src.is_file(){self.status="Select a recording before splitting".into();return;}
    let split=self.playhead;if split<=0.01 {self.status="Move the playhead past the start to split".into();return;}
    let stem=src.file_stem().unwrap_or_default().to_string_lossy();let ext=src.extension().and_then(|s|s.to_str()).unwrap_or("mp4");
    let first=self.output_dir.join(format!("{stem}_part1.{ext}"));let second=self.output_dir.join(format!("{stem}_part2.{ext}"));
    let common=["-hide_banner","-loglevel","error","-y"];
    let mut a=common.iter().map(|s|s.to_string()).collect::<Vec<_>>();a.extend(["-i".into(),src.to_string_lossy().into_owned(),"-t".into(),format!("{split:.3}"),first.to_string_lossy().into_owned()]);
    if let Err(e)=self.run_ffmpeg(a){self.status=format!("Split first half failed: {e}");return;}
    let mut b=common.iter().map(|s|s.to_string()).collect::<Vec<_>>();b.extend(["-ss".into(),format!("{split:.3}"),"-i".into(),src.to_string_lossy().into_owned(),second.to_string_lossy().into_owned()]);
    self.status=match self.run_ffmpeg(b){Ok(())=>format!("Created {} and {}",first.display(),second.display()),Err(e)=>format!("Split second half failed: {e}")};self.refresh_files();
 }
 fn seek_preview(&mut self,ctx:&egui::Context,position:f32){
    if self.chosen.is_empty() || self.preview_job.is_some(){return;}
    let path=self.ffmpeg_path.clone();let chosen=self.chosen.clone();let (tx,rx)=std::sync::mpsc::channel();self.preview_job=Some(rx);self.preview_at=position;
    let _=std::thread::spawn(move||{let output=Command::new(path).args(["-hide_banner","-loglevel","error","-ss",&format!("{position:.3}"),"-i",&chosen,"-frames:v","1","-vf","scale=800:-2","-f","image2pipe","-vcodec","png","-"]).output();let _=tx.send((position,output.map(|x|x.stdout).unwrap_or_default()));});ctx.request_repaint_after(std::time::Duration::from_millis(100));
 }
 fn poll_preview(&mut self,ctx:&egui::Context){if let Some(rx)=&self.preview_job {match rx.try_recv(){Ok((t,bytes))=>{if let Ok(im)=image::load_from_memory(&bytes){let rgba=im.to_rgba8();let size=[rgba.width() as usize,rgba.height() as usize];self.preview=Some(ctx.load_texture("selected-video-preview",egui::ColorImage::from_rgba_unmultiplied(size,rgba.as_raw()),Default::default()));if !self.is_playing{self.playhead=t;}}self.preview_job=None;},Err(std::sync::mpsc::TryRecvError::Disconnected)=>{self.preview_job=None;},Err(std::sync::mpsc::TryRecvError::Empty)=>{ctx.request_repaint_after(std::time::Duration::from_millis(100));}}}}
 fn refresh_devices(&mut self){self.speakers=audio::list_devices(false).unwrap_or_default();self.microphones=audio::list_devices(true).unwrap_or_default();self.speaker_idx=self.speaker_idx.min(self.speakers.len().saturating_sub(1));self.microphone_idx=self.microphone_idx.min(self.microphones.len().saturating_sub(1));}
 fn refresh_files(&mut self){self.assets=std::fs::read_dir(&self.output_dir).into_iter().flatten().filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().and_then(|s|s.to_str()).map(|x|matches!(x.to_ascii_lowercase().as_str(),"wav"|"mp3"|"flac"|"aac"|"m4a"|"ogg"|"mp4"|"mkv"|"webm"|"gif")).unwrap_or(false)).collect();self.assets.sort();self.assets.reverse();}
 fn start_record(&mut self){if self.recording.is_some(){return;}if self.mode==AudioMode::Off && !self.screen {self.status="Choose an audio source or enable screen capture.".into();return;}
 let t=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();let base=format!("Capture_{t}");let stop=Arc::new(AtomicBool::new(false));let mut threads=vec![];
 if matches!(self.mode,AudioMode::Speakers|AudioMode::Both){let id=self.speakers.get(self.speaker_idx).map(|x|x.1.clone());threads.push(audio::spawn_capture(false,id,self.output_dir.join(format!("{base}_speakers.wav")),stop.clone()));}
 if matches!(self.mode,AudioMode::Microphone|AudioMode::Both){let id=self.microphones.get(self.microphone_idx).map(|x|x.1.clone());threads.push(audio::spawn_capture(true,id,self.output_dir.join(format!("{base}_microphone.wav")),stop.clone()));}
 let mut video=None;
 if self.screen {let mut cmd=Command::new(&self.ffmpeg_path);cmd.args(["-hide_banner","-loglevel","error","-y","-f","gdigrab","-framerate",&self.frame_rate.to_string(),"-draw_mouse",if self.mouse{"1"}else{"0"}]);
 let [x,y,w,h]=self.region;if w>0&&h>0 {cmd.args(["-offset_x",&x.to_string(),"-offset_y",&y.to_string(),"-video_size",&format!("{w}x{h}")]);}cmd.args(["-i","desktop","-c:v","libx264","-preset","ultrafast","-pix_fmt","yuv420p","-crf","20"]);cmd.arg(self.output_dir.join(format!("{base}_screen.mp4")));cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());match cmd.spawn(){Ok(c)=>video=Some(c),Err(e)=>{stop.store(true,Ordering::Relaxed);for h in threads{let _=h.join();}self.status=format!("Screen capture could not start. Check ffmpeg.exe: {e}");return;}}}
 let clicks=Arc::new(Mutex::new(Vec::new()));
 let click_thread=if self.screen&&self.highlight_clicks{Some(monitor_clicks(stop.clone(),clicks.clone()))}else{None};
 let origin=[if self.region[2]>0{self.region[0]}else{0},if self.region[3]>0{self.region[1]}else{0}];
 self.record_started=Some(std::time::Instant::now());self.recording=Some(Recording{stop,threads,video,folder:self.output_dir.clone(),base,mode:self.mode,has_video:self.screen,clicks,click_thread,highlight:self.highlight_clicks,origin});self.status="RECORDING. Use Stop to finalize WAV and MP4 files.".into();}
 fn stop_record(&mut self){if let Some(mut rec)=self.recording.take(){self.record_started=None;rec.stop.store(true,Ordering::Relaxed);if let Some(thread)=rec.click_thread.take(){let _=thread.join();}let mut failures=vec![];for t in rec.threads.drain(..){match t.join(){Ok(Err(e))=>failures.push(e),Err(_)=>failures.push("Audio worker panicked".into()),_=>{}}}
 if let Some(mut v)=rec.video.take(){if let Some(stdin)=v.stdin.as_mut(){use std::io::Write;let _=stdin.write_all(b"q\n");}match v.wait(){Ok(st) if !st.success()=>failures.push(format!("Screen encoder exit status: {st}")),Err(e)=>failures.push(e.to_string()),_=>{}}}
 self.refresh_files();let prefix=rec.folder.join(&rec.base);let speaker=prefix.with_file_name(format!("{}_speakers.wav",rec.base));let mic=prefix.with_file_name(format!("{}_microphone.wav",rec.base));let screen=prefix.with_file_name(format!("{}_screen.mp4",rec.base));
 // Create a single usable video with explicit audio routing. Original isolated audio stems are preserved.
 if rec.has_video && rec.mode!=AudioMode::Off && screen.exists(){let out=prefix.with_file_name(format!("{}_video_with_audio.mp4",rec.base));let mut args=vec!["-y".to_string(),"-i".into(),screen.to_string_lossy().to_string()];
 if matches!(rec.mode,AudioMode::Speakers|AudioMode::Both){args.extend(["-i".into(),speaker.to_string_lossy().to_string()]);}
 if matches!(rec.mode,AudioMode::Microphone|AudioMode::Both){args.extend(["-i".into(),mic.to_string_lossy().to_string()]);}
 let filter= if rec.mode==AudioMode::Both {"[1:a][2:a]amix=inputs=2:duration=longest:normalize=0[a]"}else{"[1:a]anull[a]"};args.extend(["-filter_complex".into(),filter.into(),"-map".into(),"0:v:0".into(),"-map".into(),"[a]".into(),"-c:v".into(),"copy".into(),"-c:a".into(),"aac".into(),"-shortest".into(),out.to_string_lossy().to_string()]);if let Err(e)=self.run_ffmpeg(args){failures.push(format!("Audio/video mux: {e}"));} }
 // Produce a click-annotated copy using real click positions and recorded times.
 if rec.has_video&&rec.highlight&&screen.exists(){
 let click_log=rec.clicks.lock().map(|x|x.clone()).unwrap_or_default();
 if !click_log.is_empty(){
   let muxed=prefix.with_file_name(format!("{}_video_with_audio.mp4",rec.base));
   let source=if muxed.is_file(){muxed}else{screen.clone()};
   let target=prefix.with_file_name(format!("{}_highlighted.mp4",rec.base));
   let filters=click_log.iter().take(400).map(|(t,x,y)|{
     let xx=x-rec.origin[0];let yy=y-rec.origin[1];
     format!("drawbox=x={}:y={}:w=56:h=56:color=yellow@0.9:t=5:enable='between(t,{:.3},{:.3})'",xx-28,yy-28,t,t+0.4)
   }).collect::<Vec<_>>();
   let args=vec!["-hide_banner".into(),"-y".into(),"-i".into(),source.to_string_lossy().into_owned(),"-vf".into(),filters.join(","),"-c:v".into(),"libx264".into(),"-preset".into(),"fast".into(),"-crf".into(),"20".into(),"-c:a".into(),"copy".into(),target.to_string_lossy().into_owned()];
   if let Err(e)=self.run_ffmpeg(args){failures.push(format!("Click overlay: {e}"));}
 }
 }
 self.refresh_files();self.status=if failures.is_empty(){format!("Saved recording to {}",rec.folder.display())}else{format!("Recording ended with errors: {}",failures.join("; "))};}}
 fn run_ffmpeg(&mut self,args:Vec<String>)->Result<(),String>{let output=Command::new(&self.ffmpeg_path).args(&args).output().map_err(|e|format!("Cannot launch FFmpeg: {e}. Put ffmpeg.exe beside the app."))?;self.debug_log=format!("ffmpeg {}\n{}",args.iter().map(|v|quoted(v)).collect::<Vec<_>>().join(" "),String::from_utf8_lossy(&output.stderr));if output.status.success(){Ok(())}else{Err(self.debug_log.chars().rev().take(1100).collect::<String>().chars().rev().collect())}}
 fn do_export(&mut self){let input=PathBuf::from(self.chosen.trim());if !input.is_file(){self.status="Select an existing media file in Files or enter its path.".into();return;}let formats=["mp3","wav","flac","m4a","ogg","mp4","mkv","webm","gif"];
 let ext=formats[self.out_format];let output=if self.export.trim().is_empty(){self.output_dir.join(format!("{}_edited.{ext}",input.file_stem().unwrap_or_default().to_string_lossy()))}else{PathBuf::from(self.export.trim())};if input==output{self.status="Choose a different output name so the original remains safe.".into();return;}
 let mut args=vec!["-hide_banner".into(),"-y".into()];if self.trim_start>0. {args.extend(["-ss".into(),format!("{:.3}",self.trim_start)]);}args.extend(["-i".into(),input.to_string_lossy().to_string()]);if self.trim_end>self.trim_start && self.trim_end>0. {args.extend(["-t".into(),format!("{:.3}",self.trim_end-self.trim_start)]);}
 let is_video=["mp4","mkv","webm","gif"].contains(&ext);let mut af=vec![];
 if self.gain_db.abs()>0.05 {af.push(format!("volume={}dB",self.gain_db));}if self.bass_db.abs()>0.05{af.push(format!("bass=g={:.2}:f=120",self.bass_db));}if self.treble_db.abs()>0.05{af.push(format!("treble=g={:.2}:f=5000",self.treble_db));}if self.noise_reduce{af.push("afftdn=nf=-25".into());}if self.compressor{af.push("acompressor=threshold=0.12:ratio=3:attack=10:release=200".into());}
 if self.normalize{af.push("loudnorm=I=-16:TP=-1.5:LRA=11".into());}if self.mono{af.push("aformat=channel_layouts=mono".into());}if self.reverse{af.push("areverse".into());}
 if (self.speed-1.).abs()>0.005{let mut remaining=self.speed as f64;while remaining>2.0{af.push("atempo=2.0".into());remaining/=2.;}while remaining<0.5{af.push("atempo=0.5".into());remaining/=0.5;}af.push(format!("atempo={remaining:.6}"));}
 if self.fade_in>0. {af.push(format!("afade=t=in:st=0:d={:.2}",self.fade_in));}if self.fade_out>0. && self.trim_end>self.trim_start {let length=(self.trim_end-self.trim_start)/self.speed.max(0.25);af.push(format!("afade=t=out:st={:.2}:d={:.2}",(length-self.fade_out).max(0.),self.fade_out));}
 if is_video {let mut vf=vec![];let [x,y,w,h]=self.crop;if w>0&&h>0{vf.push(format!("crop={w}:{h}:{x}:{y}"));}if self.resize[0]>0&&self.resize[1]>0{vf.push(format!("scale={}:{}",self.resize[0],self.resize[1]));}if (self.speed-1.).abs()>0.005 {vf.push(format!("setpts=PTS/{:.5}",self.speed));}
 if self.grayscale{vf.push("hue=s=0".into());}
 if self.brightness.abs()>0.01||(self.contrast-1.).abs()>0.01||(self.saturation-1.).abs()>0.01{vf.push(format!("eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",self.brightness,self.contrast,self.saturation));}
 if self.flip_h{vf.push("hflip".into());}if self.rotation==90{vf.push("transpose=1".into());}else if self.rotation==180{vf.push("hflip".into());vf.push("vflip".into());}else if self.rotation==270{vf.push("transpose=2".into());}
 if !vf.is_empty(){args.extend(["-vf".into(),vf.join(",")]);}
 if ext=="gif" {args.extend(["-an".into(),"-loop".into(),"0".into()]);}else{if self.mute_video{args.push("-an".into());}else if !af.is_empty(){args.extend(["-af".into(),af.join(",")]);}if ext=="webm" {args.extend(["-c:v".into(),"libvpx-vp9".into(),"-c:a".into(),"libopus".into()]);}else{args.extend(["-c:v".into(),"libx264".into(),"-preset".into(),"fast".into(),"-crf".into(),"20".into(),"-pix_fmt".into(),"yuv420p".into(),"-c:a".into(),"aac".into(),"-b:a".into(),"192k".into()]);}}}
 else {args.push("-vn".into());if !af.is_empty(){args.extend(["-af".into(),af.join(",")]);}match ext{"mp3"=>args.extend(["-c:a".into(),"libmp3lame".into(),"-b:a".into(),"256k".into()]),"wav"=>args.extend(["-c:a".into(),"pcm_s24le".into()]),"flac"=>args.extend(["-c:a".into(),"flac".into()]),"m4a"=>args.extend(["-c:a".into(),"aac".into(),"-b:a".into(),"256k".into()]),"ogg"=>args.extend(["-c:a".into(),"libvorbis".into(),"-q:a".into(),"6".into()]),_=>{}}}
 args.push(output.to_string_lossy().to_string());self.status=match self.run_ffmpeg(args){Ok(())=>format!("Exported: {}",output.display()),Err(e)=>format!("Export failed: {e}")};self.refresh_files();}

 fn selected_media(&mut self, ctx:&egui::Context, path:PathBuf){
    self.stop_playback();self.chosen=path.to_string_lossy().to_string();self.preview=None;self.waveform.clear();self.media_duration=0.;self.playhead=0.;self.video_size=[0,0];self.undo.clear();self.redo.clear();self.trim_start=0.;self.trim_end=0.;
    // FFmpeg makes one preview frame and downsamples audio for the overview.  It never modifies the source.
    let probe=Command::new(&self.ffmpeg_path).args(["-hide_banner","-loglevel","error","-i",&self.chosen,"-t","1200","-f","f32le","-ac","1","-ar","400","-"]).output();
    if let Ok(o)=probe {if o.status.success(){let samples=o.stdout.chunks_exact(4).take(400*60*60*3).map(|c|f32::from_le_bytes([c[0],c[1],c[2],c[3]])).collect::<Vec<_>>();self.media_duration=samples.len() as f32/400.;let step=(samples.len()/1200).max(1);self.waveform=samples.chunks(step).map(|v|v.iter().fold(0.0f32,|a,b|a.max(b.abs()))).collect();}}
    let output=Command::new(&self.ffmpeg_path).args(["-hide_banner","-loglevel","error","-i",&self.chosen,"-frames:v","1","-vf","scale=800:-2","-f","image2pipe","-vcodec","png","-"]).output();
    if let Ok(o)=output {if o.status.success(){if let Ok(im)=image::load_from_memory(&o.stdout){let rgba=im.to_rgba8();let size=[rgba.width() as usize,rgba.height() as usize];self.preview=Some(ctx.load_texture("selected-video-preview",egui::ColorImage::from_rgba_unmultiplied(size,rgba.as_raw()),Default::default()));}}}
    if let Ok(meta)=Command::new(&self.ffmpeg_path).args(["-hide_banner","-i",&self.chosen]).output(){let st=String::from_utf8_lossy(&meta.stderr);if let Some(d)=st.split("Duration: ").nth(1).and_then(|x|x.split(',').next()){let parts=d.trim().split(':').collect::<Vec<_>>();if parts.len()==3{if let (Ok(h),Ok(m),Ok(sec))=(parts[0].parse::<f32>(),parts[1].parse::<f32>(),parts[2].parse::<f32>()){self.media_duration=h*3600.+m*60.+sec;}}}for part in st.split(|c:char|c==','||c==' '||c=='\n'){let cleaned=part.trim_matches(|c:char|!c.is_ascii_digit()&&c!='x');if let Some((w,h))=cleaned.split_once('x'){if let (Ok(w),Ok(h))=(w.parse::<i32>(),h.parse::<i32>()){if w>=160&&h>=120&&w<=16000&&h<=16000{self.video_size=[w,h];break;}}}}}
    self.status=format!("Selected: {}",path.file_name().unwrap_or_default().to_string_lossy());
 }
 fn panel(ui:&mut egui::Ui,title:&str,body:impl FnOnce(&mut egui::Ui)){
    egui::Frame::new().fill(egui::Color32::from_rgb(20,35,50)).corner_radius(9.).stroke(egui::Stroke::new(1.,egui::Color32::from_rgb(40,65,85))).inner_margin(12.).show(ui,|ui|{ui.label(egui::RichText::new(title).size(17.).strong());ui.add_space(8.);body(ui);});
 }
 fn record_top(&mut self,ui:&mut egui::Ui){
   let active=self.recording.is_some();
   ui.columns(4,|columns|{
    Self::panel(&mut columns[0],"1. Select What to Record",|ui|{
      ui.add_enabled_ui(!active,|ui|{
       ui.radio_value(&mut self.mode,AudioMode::Speakers,"Speakers only");
       ui.radio_value(&mut self.mode,AudioMode::Microphone,"Microphone only");
       ui.radio_value(&mut self.mode,AudioMode::Both,"Speakers + Microphone");
       ui.radio_value(&mut self.mode,AudioMode::Off,"No audio (screen only)");
      });ui.add_space(5.);ui.small(match self.mode{AudioMode::Speakers=>"Microphone OFF. Only Windows output audio is captured.",AudioMode::Microphone=>"Only selected microphone stream is captured.",AudioMode::Both=>"Separate speaker and microphone WAV tracks.",AudioMode::Off=>"No audio source is started."});
    });
    Self::panel(&mut columns[1],"2. Select Audio Devices",|ui|{
      ui.label("Speaker / Output Device");
      ui.add_enabled_ui(!active,|ui|{egui::ComboBox::from_id_salt("output-device").width(ui.available_width()-8.).selected_text(self.speakers.get(self.speaker_idx).map(|x|x.0.as_str()).unwrap_or("Default speaker")).show_ui(ui,|ui|{for (i,(label,_)) in self.speakers.iter().enumerate(){ui.selectable_value(&mut self.speaker_idx,i,label);}});});
      ui.label("Microphone Device");
      ui.add_enabled_ui(!active,|ui|{egui::ComboBox::from_id_salt("mic-device").width(ui.available_width()-8.).selected_text(self.microphones.get(self.microphone_idx).map(|x|x.0.as_str()).unwrap_or("Default microphone")).show_ui(ui,|ui|{for (i,(label,_)) in self.microphones.iter().enumerate(){ui.selectable_value(&mut self.microphone_idx,i,label);}});});
      if ui.add_enabled(!active,egui::Button::new("Refresh Audio Devices")).clicked(){self.refresh_devices();}
    });
    Self::panel(&mut columns[2],"3. Recording Options",|ui|{
      ui.add_enabled_ui(!active,|ui|{
        ui.checkbox(&mut self.screen,"Record Desktop / Screen");
        ui.checkbox(&mut self.mouse,"Record Cursor");
        ui.checkbox(&mut self.highlight_clicks,"Highlight mouse clicks in export");
        
        ui.label("Frame rate");egui::ComboBox::from_id_salt("frame-rate").selected_text(format!("{} FPS",self.frame_rate)).show_ui(ui,|ui|{for fps in [15,24,30,60]{ui.selectable_value(&mut self.frame_rate,fps,format!("{} FPS",fps));}});
        ui.label("Screen region (0 width = full desktop)");ui.horizontal(|ui|{for x in &mut self.region{ui.add(egui::DragValue::new(x).speed(2));}});
      });
    });
    Self::panel(&mut columns[3],"4. Start Recording",|ui|{
      if !active {if ui.add_sized([ui.available_width(),48.],egui::Button::new(egui::RichText::new("●  START RECORDING").size(18.).strong().color(egui::Color32::WHITE)).fill(egui::Color32::from_rgb(225,37,46))).clicked(){self.start_record();}}
      else {ui.add_enabled(false,egui::Button::new("RECORDING IN PROGRESS"));}
      if ui.add_enabled(active,egui::Button::new(egui::RichText::new("■  STOP & SAVE").size(16.).strong())).clicked(){self.stop_record();}
      let secs=self.record_started.map(|x|x.elapsed().as_secs()).unwrap_or(0);ui.vertical_centered(|ui|{ui.label(egui::RichText::new(format!("{:02}:{:02}:{:02}",secs/3600,(secs/60)%60,secs%60)).size(30.).strong());ui.colored_label(if active {egui::Color32::LIGHT_RED}else{egui::Color32::LIGHT_GREEN},if active{"● Recording"}else{"● Ready to record"});});
      if active{ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));}
    });
   });
 }
 fn library(&mut self,ui:&mut egui::Ui,ctx:&egui::Context){
   ui.horizontal(|ui|{ui.label(egui::RichText::new(format!("Recorded Files ({})",self.assets.len())).size(16.).strong());if ui.button("Refresh").clicked(){self.refresh_files();}if ui.button("Open Folder").clicked(){let _=Command::new("explorer.exe").arg(&self.output_dir).spawn();}if ui.button("Open Media File...").clicked(){if let Some(path)=Self::pick_file(false,"Media Files|*.wav;*.mp3;*.flac;*.m4a;*.ogg;*.mp4;*.mkv;*.webm;*.gif|All Files|*.*"){self.selected_media(ctx,PathBuf::from(path));}}});
   ui.separator();
   let paths=self.assets.clone();egui::ScrollArea::vertical().max_height(360.).show(ui,|ui|{for p in paths{let chosen=self.chosen==p.to_string_lossy();let name=p.file_name().unwrap_or_default().to_string_lossy();if ui.selectable_label(chosen,format!("{}  {}",if matches!(p.extension().and_then(|e|e.to_str()),Some("mp4"|"mkv"|"webm"|"gif")){"▣"}else{"♫"},name)).clicked(){self.selected_media(ctx,p);}}});
 }
 fn waveform_view(&mut self,ui:&mut egui::Ui){
   ui.label(egui::RichText::new("Audio Waveform / Trim Selection").size(16.).strong());
   let width=ui.available_width().max(120.);let (rect,response)=ui.allocate_exact_size(egui::vec2(width,165.),egui::Sense::click_and_drag());
   ui.painter().rect_filled(rect,6.,egui::Color32::from_rgb(7,22,35));
   let mid=rect.center().y;
   for (i,a) in self.waveform.iter().enumerate(){let x=rect.left()+i as f32/self.waveform.len().max(1) as f32*rect.width();let amplitude=a.min(1.)*65.;ui.painter().line_segment([egui::pos2(x,mid-amplitude),egui::pos2(x,mid+amplitude)],egui::Stroke::new(1.,egui::Color32::from_rgb(25,174,244)));}
   if self.media_duration>0. {
    let start=(self.trim_start/self.media_duration).clamp(0.,1.);let end=if self.trim_end<=0. {1.} else {(self.trim_end/self.media_duration).clamp(0.,1.)};
    for (f,color) in [(start,egui::Color32::LIGHT_GREEN),(end,egui::Color32::YELLOW)]{let x=rect.left()+rect.width()*f;ui.painter().line_segment([egui::pos2(x,rect.top()),egui::pos2(x,rect.bottom())],egui::Stroke::new(2.,color));}
    if response.drag_started()||response.clicked(){self.save_undo();}if response.dragged()||response.clicked(){if let Some(pos)=response.interact_pointer_pos(){let time=((pos.x-rect.left())/rect.width()).clamp(0.,1.)*self.media_duration;if ui.input(|i|i.modifiers.shift){self.trim_end=time.max(self.trim_start);}else{self.trim_start=time.min(if self.trim_end>0.{self.trim_end}else{self.media_duration});}}}
   }
   ui.small("Click waveform to set trim start. Shift-click or Shift-drag to set trim end.");
   ui.horizontal(|ui|{ui.label("Start (sec)");ui.add(egui::DragValue::new(&mut self.trim_start).range(0.0..=360000.0).speed(0.1));ui.label("End (0=full)");ui.add(egui::DragValue::new(&mut self.trim_end).range(0.0..=360000.0).speed(0.1));});
 }
 fn preview_view(&mut self,ui:&mut egui::Ui){
   ui.label(egui::RichText::new("Video Preview / Visual Crop").size(16.).strong());
   if let Some(t)=&self.preview{let sz=t.size_vec2();let width=ui.available_width().min(520.);let height=(width*sz.y/sz.x).min(255.);let sz=egui::vec2(width,height);let (rect,response)=ui.allocate_exact_size(sz,egui::Sense::click_and_drag());ui.painter().image(t.id(),rect,egui::Rect::from_min_max(egui::pos2(0.,0.),egui::pos2(1.,1.)),egui::Color32::WHITE);
    if self.crop[2]>0&&self.crop[3]>0&&self.video_size[0]>0&&self.video_size[1]>0{let [x,y,w,h]=self.crop;let a=egui::pos2(rect.left()+rect.width()*x as f32/self.video_size[0] as f32,rect.top()+rect.height()*y as f32/self.video_size[1] as f32);let b=egui::pos2(rect.left()+rect.width()*(x+w) as f32/self.video_size[0] as f32,rect.top()+rect.height()*(y+h) as f32/self.video_size[1] as f32);ui.painter().rect_stroke(egui::Rect::from_two_pos(a,b),0.,egui::Stroke::new(2.,egui::Color32::YELLOW),egui::StrokeKind::Outside);}
    if response.drag_started(){self.crop_drag_start=response.interact_pointer_pos();self.save_undo();}
    if response.dragged(){if let (Some(a),Some(b))=(self.crop_drag_start,response.interact_pointer_pos()){if self.video_size[0]>0&&self.video_size[1]>0{let fx=|p:f32|((p-rect.left())/rect.width()).clamp(0.,1.);let fy=|p:f32|((p-rect.top())/rect.height()).clamp(0.,1.);let x1=(fx(a.x.min(b.x))*self.video_size[0] as f32) as i32;let x2=(fx(a.x.max(b.x))*self.video_size[0] as f32) as i32;let y1=(fy(a.y.min(b.y))*self.video_size[1] as f32) as i32;let y2=(fy(a.y.max(b.y))*self.video_size[1] as f32) as i32;self.crop=[x1,y1,(x2-x1).max(2)&!1,(y2-y1).max(2)&!1];}}}
    if response.drag_stopped(){self.crop_drag_start=None;}
   }else{ui.label("Select a video to preview, or an audio file to edit its waveform.");}
   ui.small("Drag a rectangle on the video to crop. Crop dimensions follow original video pixels.");
   if self.media_duration>0.{let mut pos=self.playhead;let changed=ui.add(egui::Slider::new(&mut pos,0.0..=self.media_duration).text("Position (s)")).changed();if changed{self.stop_playback();self.playhead=pos;self.seek_preview(ui.ctx(),pos);}ui.horizontal(|ui|{if ui.button("◀ Frame -1s").clicked(){self.playhead=(self.playhead-1.).max(0.);self.seek_preview(ui.ctx(),self.playhead);}if ui.button("Preview at Position").clicked(){self.seek_preview(ui.ctx(),self.playhead);}if ui.button("Frame +1s ▶").clicked(){self.playhead=(self.playhead+1.).min(self.media_duration);self.seek_preview(ui.ctx(),self.playhead);}});}
   ui.horizontal(|ui|{if !self.is_playing{if ui.add_enabled(!self.chosen.is_empty(),egui::Button::new("▶ PLAY HERE")).clicked(){self.start_playback();}}else if ui.button("⏸ PAUSE").clicked(){self.stop_playback();}if ui.button("■ STOP").clicked(){self.stop_playback();self.playhead=0.;} });
 }
 fn quick_edit(&mut self,ui:&mut egui::Ui){
   ui.label(egui::RichText::new("Quick Tools").size(16.).strong());
   ui.horizontal_wrapped(|ui|{
      if ui.button("✂ Trim").clicked(){self.tab=Tab::Edit;}
      if ui.button("◖ Volume").clicked(){self.tab=Tab::Edit;}
      if ui.button("▣ Crop").clicked(){self.tab=Tab::Edit;}
      if ui.button("⫶ Split into 2 files").clicked(){self.split_at_playhead();}
      if ui.button("↶ Reset Edits").clicked(){self.reset_edits();}if ui.button("↶ Undo").clicked(){self.undo_edit();}if ui.button("↷ Redo").clicked(){self.redo_edit();}
   });
 }
 fn workspace(&mut self,ui:&mut egui::Ui,ctx:&egui::Context){
    ui.horizontal(|ui|{ui.selectable_value(&mut self.tab,Tab::Files,"▣  Recordings");ui.selectable_value(&mut self.tab,Tab::Edit,"♫  Audio Editor / Video Editor");ui.selectable_value(&mut self.tab,Tab::Record,"●  Recorder");});
    ui.separator();
    ui.columns(3,|cs|{
      Self::panel(&mut cs[0],"MEDIA LIBRARY",|ui|self.library(ui,ctx));
      Self::panel(&mut cs[1],"PREVIEW & TOOLS",|ui|{self.preview_view(ui);ui.separator();self.quick_edit(ui);});
      Self::panel(&mut cs[2],"WAVEFORM & EXPORT",|ui|{
        self.waveform_view(ui);ui.separator();ui.horizontal(|ui|{ui.label("Format");let formats=["mp3","wav","flac","m4a","ogg","mp4","mkv","webm","gif"];egui::ComboBox::from_id_salt("quick-format").selected_text(formats[self.out_format]).show_ui(ui,|ui|{for (i,x) in formats.iter().enumerate(){ui.selectable_value(&mut self.out_format,i,*x);}});});
        if ui.add_sized([ui.available_width(),35.],egui::Button::new("EXPORT SELECTED").fill(egui::Color32::from_rgb(16,103,215))).clicked(){self.do_export();}
      });
    });
 }
 fn editor_controls(&mut self,ui:&mut egui::Ui){
   let before=self.snapshot();
   if ui.button("Save edit checkpoint (for Undo)").clicked(){self.save_undo();}egui::CollapsingHeader::new("Advanced Audio Editing & Effects").default_open(true).show(ui,|ui|{
      ui.horizontal_wrapped(|ui|{ui.label("Volume dB");ui.add(egui::Slider::new(&mut self.gain_db,-30.0..=24.0));ui.label("Speed");ui.add(egui::Slider::new(&mut self.speed,0.25..=4.0));});
      ui.horizontal(|ui|{ui.label("Fade in (sec)");ui.add(egui::DragValue::new(&mut self.fade_in));ui.label("Fade out (sec)");ui.add(egui::DragValue::new(&mut self.fade_out));});
      ui.horizontal_wrapped(|ui|{ui.checkbox(&mut self.normalize,"Normalize loudness");ui.checkbox(&mut self.reverse,"Reverse audio");ui.checkbox(&mut self.mono,"Convert to mono");});
      ui.horizontal_wrapped(|ui|{ui.label("Bass (dB)");ui.add(egui::Slider::new(&mut self.bass_db,-15.0..=15.0));ui.label("Treble (dB)");ui.add(egui::Slider::new(&mut self.treble_db,-15.0..=15.0));});
      ui.horizontal_wrapped(|ui|{ui.checkbox(&mut self.noise_reduce,"Reduce background noise");ui.checkbox(&mut self.compressor,"Dynamic compressor");});
   });
   egui::CollapsingHeader::new("Video Crop / Resize / Export Settings").default_open(true).show(ui,|ui|{
      ui.checkbox(&mut self.mute_video,"Remove video audio");ui.label("Crop X / Y / Width / Height (0 = no crop)");ui.horizontal(|ui|{for x in &mut self.crop{ui.add(egui::DragValue::new(x).speed(2));}});
      ui.label("Resize Width / Height (0 = original)");ui.horizontal(|ui|{for x in &mut self.resize{ui.add(egui::DragValue::new(x).speed(2));}});
      ui.horizontal_wrapped(|ui|{ui.checkbox(&mut self.grayscale,"Grayscale");ui.checkbox(&mut self.flip_h,"Flip horizontally");ui.label("Rotate");egui::ComboBox::from_id_salt("rotate").selected_text(format!("{}°",self.rotation)).show_ui(ui,|ui|{for n in [0,90,180,270]{ui.selectable_value(&mut self.rotation,n,format!("{n}°"));}});});
      ui.horizontal_wrapped(|ui|{ui.label("Brightness");ui.add(egui::Slider::new(&mut self.brightness,-0.5..=0.5));ui.label("Contrast");ui.add(egui::Slider::new(&mut self.contrast,0.5..=2.));ui.label("Saturation");ui.add(egui::Slider::new(&mut self.saturation,0.0..=3.));});
      ui.label("Output full path (blank = automatic)");ui.horizontal(|ui|{ui.text_edit_singleline(&mut self.export);if ui.button("Browse Save As...").clicked(){if let Some(path)=Self::pick_file(true,"Video|*.mp4;*.mkv;*.webm;*.gif|Audio|*.mp3;*.wav;*.flac;*.m4a;*.ogg|All Files|*.*"){self.export=path;}}});
   });
   let after=self.snapshot();if after!=before{self.undo.push(before);if self.undo.len()>75{self.undo.remove(0);}self.redo.clear();}
 }
}
impl Drop for Studio {fn drop(&mut self){self.stop_playback();if self.recording.is_some(){self.stop_record();}}}
impl eframe::App for Studio {
 fn ui(&mut self,ui:&mut egui::Ui,_frame:&mut eframe::Frame){
  ui.ctx().set_visuals(egui::Visuals::dark());
  ui.style_mut().visuals.override_text_color=Some(egui::Color32::from_rgb(225,240,255));
  ui.style_mut().visuals.panel_fill=egui::Color32::from_rgb(8,20,32);
  ui.style_mut().visuals.widgets.inactive.bg_fill=egui::Color32::from_rgb(29,50,69);
  ui.style_mut().visuals.widgets.inactive.fg_stroke.color=egui::Color32::WHITE;
  ui.style_mut().visuals.widgets.hovered.bg_fill=egui::Color32::from_rgb(35,94,141);
  ui.style_mut().spacing.item_spacing=egui::vec2(8.,9.);
  ui.style_mut().spacing.button_padding=egui::vec2(10.,7.);

  egui::Frame::new().fill(egui::Color32::from_rgb(4,28,52)).inner_margin(12.).show(ui,|ui|{ui.horizontal(|ui|{ui.vertical(|ui|{ui.label(egui::RichText::new("MORRISON").size(31.).strong().color(egui::Color32::WHITE));ui.label(egui::RichText::new("CAPTURE STUDIO").size(22.).strong().color(egui::Color32::from_rgb(75,194,250)));});ui.separator();ui.label("Rust Edition\nAudio + Screen Recording\nEditing + Media Export");});});
  ui.add_space(10.);
  egui::ScrollArea::vertical().show(ui,|ui|{
    self.poll_preview(ui.ctx());self.update_playback(ui.ctx());self.record_top(ui);ui.add_space(12.);let context=ui.ctx().clone();self.workspace(ui,&context);
    if self.tab==Tab::Edit{ui.add_space(10.);self.editor_controls(ui);ui.add_space(10.);self.multitrack_editor(ui);ui.add_space(10.);self.video_timeline_ui(ui);}
    ui.add_space(8.);egui::CollapsingHeader::new("Save / Open Editing Project").show(ui,|ui|{ui.horizontal(|ui|{ui.label("Project file");ui.text_edit_singleline(&mut self.project_path);if ui.button("Save Project").clicked(){self.save_project();}if ui.button("Save Project As...").clicked(){if let Some(path)=Self::pick_file(true,"Morrison Studio Project|*.json"){self.project_path=path;self.save_project();}}if ui.button("Open Project...").clicked(){if let Some(path)=Self::pick_file(false,"Morrison Studio Project|*.json"){self.project_path=path;self.load_project(ui.ctx());}}});});
    ui.add_space(12.);ui.separator();ui.horizontal_wrapped(|ui|{ui.colored_label(egui::Color32::LIGHT_GREEN,&self.status);ui.label(format!("Output: {}",self.output_dir.display()));if ui.button("Open Output Folder").clicked(){let _=Command::new("explorer.exe").arg(&self.output_dir).spawn();}if ui.button("Change Output Folder...").clicked(){if let Some(dir)=Self::choose_folder(){self.output_dir=dir;self.refresh_files();}}});
  });
 }
}
fn main()->eframe::Result<()> {let opts=eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([1536.0,1018.0]).with_min_inner_size([1130.0,760.0]).with_title("Morrison Capture Studio v5"),..Default::default()};eframe::run_native("Morrison Capture Studio v5",opts,Box::new(|_|Ok(Box::new(Studio::default()))))}
