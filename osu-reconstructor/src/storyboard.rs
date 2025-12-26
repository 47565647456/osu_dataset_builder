//! Storyboard reconstruction from parquet rows
//!
//! Note: This module provides storyboard element reconstruction info.
//! Full storyboard reconstruction requires the rosu-storyboard encoding API.

use std::collections::HashMap;
use crate::types::*;

/// Reconstructor for storyboard elements
pub struct StoryboardReconstructor;

/// Reconstructed storyboard element with its commands
#[derive(Debug, Clone)]
pub struct ReconstructedElement {
    pub layer_name: String,
    pub element_path: String,
    pub element_type: String,
    pub origin: String,
    pub initial_pos_x: f32,
    pub initial_pos_y: f32,
    pub frame_count: Option<i32>,
    pub frame_delay: Option<f64>,
    pub loop_type: Option<String>,
    pub is_embedded: bool,
    pub commands: Vec<ReconstructedCommand>,
    pub loops: Vec<ReconstructedLoop>,
    pub triggers: Vec<ReconstructedTrigger>,
}

/// Reconstructed storyboard command
#[derive(Debug, Clone)]
pub struct ReconstructedCommand {
    pub command_type: String,
    pub easing: i32,
    pub start_time: f64,
    pub end_time: f64,
    pub start_value: String,
    pub end_value: String,
}

/// Reconstructed storyboard loop
#[derive(Debug, Clone)]
pub struct ReconstructedLoop {
    pub loop_start_time: f64,
    pub loop_count: i32,
}

/// Reconstructed storyboard trigger
#[derive(Debug, Clone)]
pub struct ReconstructedTrigger {
    pub trigger_name: String,
    pub trigger_start_time: f64,
    pub trigger_end_time: f64,
    pub group_number: i32,
}

impl StoryboardReconstructor {
    /// Reconstruct storyboard elements for a specific folder+file
    pub fn reconstruct(
        folder_id: &str,
        source_file: &str,
        element_rows: &[StoryboardElementRow],
        command_rows: &[StoryboardCommandRow],
        loop_rows: &[StoryboardLoopRow],
        trigger_rows: &[StoryboardTriggerRow],
    ) -> Vec<ReconstructedElement> {
        // Filter elements for this file
        let matching_elements: Vec<_> = element_rows
            .iter()
            .filter(|e| e.folder_id == folder_id && e.source_file == source_file)
            .collect();

        // Group commands by element_index
        let mut commands_by_element: HashMap<i32, Vec<&StoryboardCommandRow>> = HashMap::new();
        for cmd in command_rows
            .iter()
            .filter(|c| c.folder_id == folder_id && c.source_file == source_file)
        {
            commands_by_element.entry(cmd.element_index).or_default().push(cmd);
        }

        // Group loops by element_index
        let mut loops_by_element: HashMap<i32, Vec<&StoryboardLoopRow>> = HashMap::new();
        for lp in loop_rows
            .iter()
            .filter(|l| l.folder_id == folder_id && l.source_file == source_file)
        {
            loops_by_element.entry(lp.element_index).or_default().push(lp);
        }

        // Group triggers by element_index
        let mut triggers_by_element: HashMap<i32, Vec<&StoryboardTriggerRow>> = HashMap::new();
        for tr in trigger_rows
            .iter()
            .filter(|t| t.folder_id == folder_id && t.source_file == source_file)
        {
            triggers_by_element.entry(tr.element_index).or_default().push(tr);
        }

        // Build reconstructed elements
        matching_elements
            .iter()
            .map(|elem| {
                let commands = commands_by_element
                    .get(&elem.element_index)
                    .map(|cmds| {
                        cmds.iter()
                            .map(|c| ReconstructedCommand {
                                command_type: c.command_type.clone(),
                                easing: c.easing,
                                start_time: c.start_time,
                                end_time: c.end_time,
                                start_value: c.start_value.clone(),
                                end_value: c.end_value.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let loops = loops_by_element
                    .get(&elem.element_index)
                    .map(|lps| {
                        lps.iter()
                            .map(|l| ReconstructedLoop {
                                loop_start_time: l.loop_start_time,
                                loop_count: l.loop_count,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let triggers = triggers_by_element
                    .get(&elem.element_index)
                    .map(|trs| {
                        trs.iter()
                            .map(|t| ReconstructedTrigger {
                                trigger_name: t.trigger_name.clone(),
                                trigger_start_time: t.trigger_start_time,
                                trigger_end_time: t.trigger_end_time,
                                group_number: t.group_number,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                ReconstructedElement {
                    layer_name: elem.layer_name.clone(),
                    element_path: elem.element_path.clone(),
                    element_type: elem.element_type.clone(),
                    origin: elem.origin.clone(),
                    initial_pos_x: elem.initial_pos_x,
                    initial_pos_y: elem.initial_pos_y,
                    frame_count: elem.frame_count,
                    frame_delay: elem.frame_delay,
                    loop_type: elem.loop_type.clone(),
                    is_embedded: elem.is_embedded,
                    commands,
                    loops,
                    triggers,
                }
            })
            .collect()
    }

    /// Get unique storyboard files in a folder (embedded .osu files won't appear here)
    pub fn get_storyboard_files(folder_id: &str, element_rows: &[StoryboardElementRow]) -> Vec<String> {
        let mut files: Vec<String> = element_rows
            .iter()
            .filter(|e| e.folder_id == folder_id && !e.is_embedded)
            .map(|e| e.source_file.clone())
            .collect();
        files.sort();
        files.dedup();
        files
    }

    /// Get embedded storyboard source files (the .osu files that have embedded storyboards)
    pub fn get_embedded_storyboard_files(folder_id: &str, element_rows: &[StoryboardElementRow]) -> Vec<String> {
        let mut files: Vec<String> = element_rows
            .iter()
            .filter(|e| e.folder_id == folder_id && e.is_embedded)
            .map(|e| e.source_file.clone())
            .collect();
        files.sort();
        files.dedup();
        files
    }

    /// Write storyboard elements to .osb format
    /// This generates the basic storyboard script format
    pub fn to_osb_content(elements: &[ReconstructedElement]) -> String {
        let mut output = String::new();
        output.push_str("[Events]\n");
        output.push_str("//Background and Video events\n");
        // Note: Background image and breaks are handled by beatmap, not here
        output.push_str("//Storyboard Layer 0 (Background)\n");
        Self::write_layer_elements(&mut output, elements, "Background");
        output.push_str("//Storyboard Layer 1 (Fail)\n");
        Self::write_layer_elements(&mut output, elements, "Fail");
        output.push_str("//Storyboard Layer 2 (Pass)\n");
        Self::write_layer_elements(&mut output, elements, "Pass");
        output.push_str("//Storyboard Layer 3 (Foreground)\n");
        Self::write_layer_elements(&mut output, elements, "Foreground");
        // Overlay layer if present
        let has_overlay = elements.iter().any(|e| e.layer_name == "Overlay");
        if has_overlay {
            output.push_str("//Storyboard Layer 4 (Overlay)\n");
            Self::write_layer_elements(&mut output, elements, "Overlay");
        }
        
        output
    }

    fn write_layer_elements(output: &mut String, elements: &[ReconstructedElement], layer: &str) {
        for elem in elements.iter().filter(|e| e.layer_name == layer) {
            Self::write_element(output, elem, layer);
        }
    }

    /// Generate storyboard content for embedding in a .osu file
    pub fn to_embedded_events_content(elements: &[ReconstructedElement]) -> String {
        Self::to_osb_content(elements)
    }

    fn write_element(output: &mut String, elem: &ReconstructedElement, layer_name: &str) {
        match elem.element_type.as_str() {
            "sprite" => {
                // Sprite format: Sprite,layer,origin,filepath,x,y
                let origin = Self::parse_origin(&elem.origin);
                output.push_str(&format!(
                    "Sprite,{},{},\"{}\",{},{}\n",
                    layer_name, origin, elem.element_path, 
                    elem.initial_pos_x as i32, elem.initial_pos_y as i32
                ));
                
                // Write commands
                for cmd in &elem.commands {
                    let cmd_str = Self::format_command(cmd);
                    output.push_str(&format!(" {}\n", cmd_str));
                }

                // Write loops
                for lp in &elem.loops {
                    output.push_str(&format!(" L,{},{}\n", lp.loop_start_time as i32, lp.loop_count));
                }

                // Write triggers
                for tr in &elem.triggers {
                    output.push_str(&format!(" T,{},{},{},{}\n", 
                        tr.trigger_name, 
                        tr.trigger_start_time as i32, 
                        tr.trigger_end_time as i32,
                        tr.group_number
                    ));
                }
            }
            "animation" => {
                // Animation format: Animation,layer,origin,filepath,x,y,frameCount,frameDelay,loopType
                let origin = Self::parse_origin(&elem.origin);
                let frame_count = elem.frame_count.unwrap_or(1);
                let frame_delay = elem.frame_delay.unwrap_or(100.0);
                let loop_type = elem.loop_type.as_deref().unwrap_or("LoopForever");
                
                output.push_str(&format!(
                    "Animation,{},{},\"{}\",{},{},{},{},{}\n",
                    layer_name, origin, elem.element_path,
                    elem.initial_pos_x as i32, elem.initial_pos_y as i32,
                    frame_count, frame_delay as i32, loop_type
                ));
                
                for cmd in &elem.commands {
                    let cmd_str = Self::format_command(cmd);
                    output.push_str(&format!(" {}\n", cmd_str));
                }

                // Write loops
                for lp in &elem.loops {
                    output.push_str(&format!(" L,{},{}\n", lp.loop_start_time as i32, lp.loop_count));
                }

                // Write triggers  
                for tr in &elem.triggers {
                    output.push_str(&format!(" T,{},{},{},{}\n",
                        tr.trigger_name,
                        tr.trigger_start_time as i32,
                        tr.trigger_end_time as i32,
                        tr.group_number
                    ));
                }
            }
            "sample" => {
                // Sample format: Sample,time,layer,filepath,volume
                // We don't have full sample data, so skip for now
            }
            _ => {}
        }
    }

    fn parse_origin(origin_str: &str) -> &'static str {
        if origin_str.contains("TopLeft") { "TopLeft" }
        else if origin_str.contains("TopCentre") || origin_str.contains("TopCenter") { "TopCentre" }
        else if origin_str.contains("TopRight") { "TopRight" }
        else if origin_str.contains("CentreLeft") || origin_str.contains("CenterLeft") { "CentreLeft" }
        else if origin_str.contains("Centre") || origin_str.contains("Center") { "Centre" }
        else if origin_str.contains("CentreRight") || origin_str.contains("CenterRight") { "CentreRight" }
        else if origin_str.contains("BottomLeft") { "BottomLeft" }
        else if origin_str.contains("BottomCentre") || origin_str.contains("BottomCenter") { "BottomCentre" }
        else if origin_str.contains("BottomRight") { "BottomRight" }
        else { "Centre" }
    }

    fn format_command(cmd: &ReconstructedCommand) -> String {
        // Handle P (parameter) command specially
        if cmd.command_type == "blending" || cmd.command_type == "flip_h" || cmd.command_type == "flip_v" {
            // P command format: P,easing,startTime,endTime,parameter
            // parameter: H=flip horizontal, V=flip vertical, A=additive blending
            let param = if cmd.command_type == "flip_h" {
                "H"
            } else if cmd.command_type == "flip_v" {
                "V"
            } else {
                // blending_parameters - check if it's additive (contains "SrcAlpha" and "One")
                if cmd.start_value.contains("SrcAlpha") && cmd.start_value.contains("One") {
                    "A"
                } else {
                    "A" // Default to additive
                }
            };
            
            // For P command, if start_time == end_time, omit end_time
            let start_time = cmd.start_time as i32;
            let end_time = cmd.end_time as i32;
            if start_time == end_time {
                return format!("P,{},{},{},{}", cmd.easing, start_time, "", param);
            } else {
                return format!("P,{},{},{},{}", cmd.easing, start_time, end_time, param);
            }
        }
        
        // Command format: Type,easing,startTime,endTime,startValue,endValue
        let cmd_code = match cmd.command_type.as_str() {
            "x" => "MX",
            "y" => "MY",
            "scale" => "S",
            "rotation" => "R",
            "alpha" => "F",
            "color" => "C",
            "vector_scale" => "V",
            _ => "_",
        };
        
        format!(
            "{},{},{},{},{}",
            cmd_code,
            cmd.easing,
            cmd.start_time as i32,
            cmd.end_time as i32,
            Self::format_values(&cmd.start_value, &cmd.end_value)
        )
    }

    fn format_values(start: &str, end: &str) -> String {
        if start == end {
            start.to_string()
        } else {
            format!("{},{}", start, end)
        }
    }
}

