pub mod ass_writer;
pub mod canvas;
pub mod danmaku;
pub mod drawable;

use std::{cmp::Ordering, fs::File};

use ass_writer::AssWriter;
use canvas::CanvasConfig;
use danmaku::{Danmaku, DanmakuType};
use eyre::eyre;
use tracing::instrument;
use yaserde::{YaDeserialize, YaSerialize};

#[derive(YaSerialize, YaDeserialize)]
#[yaserde(rename = "d")]
pub struct DamakuXmlDTag {
    #[yaserde(attribute = true)]
    pub p: String,
    #[yaserde(text = true)]
    pub body: Option<String>,
}

#[derive(YaSerialize, YaDeserialize)]
#[yaserde(rename = "i")]
pub struct DanmakuXmlITag {
    pub chatid: i64,
    #[yaserde(rename = "d")]
    pub elems: Vec<DamakuXmlDTag>,
}

#[instrument(level = "error", skip_all)]
pub fn xml_to_ass(
    xml: &str,
    ass_file: File,
    title: String,
    config: CanvasConfig,
) -> eyre::Result<()> {
    let mut writer = AssWriter::new(ass_file, title, config.clone())?;
    let mut canvas = config.canvas();

    let mut danmakus: Vec<Danmaku> = xml_to_danmakus(xml)?;
    danmakus.sort_by(|a, b| {
        a.timeline_s
            .partial_cmp(&b.timeline_s)
            .unwrap_or(Ordering::Equal)
    });

    for danmaku in danmakus {
        if let Some(drawable) = canvas.draw(danmaku) {
            writer.write(drawable)?;
        }
    }

    Ok(())
}

trait ToDanmakuType {
    fn to_danmaku_type(&self) -> eyre::Result<DanmakuType>;
}

impl ToDanmakuType for u32 {
    #[instrument(level = "error", skip_all)]
    fn to_danmaku_type(&self) -> eyre::Result<DanmakuType> {
        match self {
            1 => Ok(DanmakuType::Float),
            4 => Ok(DanmakuType::Bottom),
            5 => Ok(DanmakuType::Top),
            6 => Ok(DanmakuType::Reverse),
            _ => Err(eyre!("未知的弹幕类型：{self}")),
        }
    }
}

#[instrument(level = "error", skip_all)]
pub fn xml_to_danmakus(xml: &str) -> eyre::Result<Vec<Danmaku>> {
    let xml = sanitize_xml(xml);
    let i_tag: DanmakuXmlITag = yaserde::de::from_str(&xml).map_err(|e| eyre!(e))?;

    let mut danmakus = Vec::new();

    for elem in i_tag.elems {
        let Some(content) = elem.body else {
            continue;
        };

        let mut p_attr = elem.p.split(',');

        let Some(timeline_s) = p_attr.next().and_then(|s| s.parse::<f64>().ok()) else {
            return Err(eyre!("弹幕`{content}`的p属性中没有时间"));
        };

        let Some(r#type) = p_attr
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .and_then(|num| num.to_danmaku_type().ok())
        else {
            return Err(eyre!("弹幕`{content}`的p属性中没有弹幕类型"));
        };

        let Some(fontsize) = p_attr.next().and_then(|s| s.parse::<u32>().ok()) else {
            return Err(eyre!("弹幕`{content}`的p属性中没有字体大小"));
        };

        let Some(rgb) = p_attr.next().and_then(|s| s.parse::<u32>().ok()) else {
            return Err(eyre!("弹幕`{content}`的p属性中没有颜色"));
        };

        // rgb 是个数字，类似 0x010203
        let r = (rgb >> 16) & 0xff;
        let g = (rgb >> 8) & 0xff;
        let b = rgb & 0xff;

        let danmaku = Danmaku {
            timeline_s,
            content,
            r#type,
            fontsize,
            rgb: (r as u8, g as u8, b as u8),
        };

        danmakus.push(danmaku);
    }

    Ok(danmakus)
}

fn sanitize_xml(s: &str) -> String {
    fn is_valid_xml_char(c: char) -> bool {
        matches!(c,
            '\u{0009}' |
            '\u{000A}' |
            '\u{000D}' |
            '\u{0020}'..='\u{D7FF}' |
            '\u{E000}'..='\u{FFFD}' |
            '\u{10000}'..='\u{10FFFF}'
        )
    }
    s.chars().filter(|&c| is_valid_xml_char(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entities_cdata_unicode_and_ignores_unrelated_metadata() {
        let xml = r#"<i><chatid>42</chatid><maxlimit>1000</maxlimit><d p="1.25,1,25,16711680,0,0,x,1">你好 &amp; 👋</d><d p="2,5,36,255,0,0,x,2"><![CDATA[a < b]]></d></i>"#;
        let parsed = xml_to_danmakus(xml).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].content, "你好 & 👋");
        assert_eq!(parsed[0].timeline_s, 1.25);
        assert_eq!(parsed[0].rgb, (255, 0, 0));
        assert_eq!(parsed[1].content, "a < b");
        assert_eq!(parsed[1].r#type, DanmakuType::Top);
    }

    #[test]
    fn serialized_danmaku_round_trips_and_removes_invalid_xml_controls() {
        let model = DanmakuXmlITag {
            chatid: 42,
            elems: vec![DamakuXmlDTag {
                p: "0,4,25,16777215".into(),
                body: Some("<hello> & 世界".into()),
            }],
        };
        let xml = yaserde::ser::to_string(&model).unwrap();
        let parsed = xml_to_danmakus(&xml).unwrap();
        assert_eq!(parsed[0].content, "<hello> & 世界");
        let with_control = xml.replace("世界", "世\u{0001}界");
        assert_eq!(xml_to_danmakus(&with_control).unwrap(), parsed);
    }

    #[test]
    fn malformed_xml_and_invalid_metadata_return_errors() {
        for xml in [
            "<i><d>",
            r#"<i><chatid>1</chatid><d>missing p</d></i>"#,
            r#"<i><chatid>1</chatid><d p="bad,1,25,255">x</d></i>"#,
        ] {
            assert!(xml_to_danmakus(xml).is_err(), "{xml}");
        }
        assert!(
            xml_to_danmakus("<i><chatid>1</chatid></i>")
                .unwrap()
                .is_empty()
        );
    }
}
