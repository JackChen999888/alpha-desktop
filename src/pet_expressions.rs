//! Alpha 表情调度：按场景播放，隐藏时暂停，日常动作之间留出安静时间。
use crate::pet::PetState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Expression {
    Wink, Smug, Blush, Delighted, SideGlance, Praised,
    Heart, Giggle, ShyPeek, Scan, Peek, Search,
}

impl Expression {
    pub const ALL:[Self;12]=[Self::Wink,Self::Smug,Self::Blush,Self::Delighted,Self::SideGlance,Self::Praised,
        Self::Heart,Self::Giggle,Self::ShyPeek,Self::Scan,Self::Peek,Self::Search];
    fn times(self)->[f32;16] {
        if self as usize>=9 {[0.35,0.13,0.15,0.18,0.35,0.28,0.16,0.11,0.11,0.16,0.35,0.28,0.16,0.14,0.20,0.35]}
        else {[0.35,0.12,0.12,0.12,0.13,0.13,0.14,0.40,0.18,0.14,0.12,0.12,0.12,0.14,0.14,0.45]}
    }
    pub fn duration(self)->f32 {self.times().iter().sum()}

}

pub(crate) struct Expressions {
    clip:Option<(Expression,f32)>,
    wait:f32,
    cooldown:f32,
    hovered:bool,
    next:[usize;4],
}

impl Default for Expressions {
    fn default()->Self {Self{clip:None,wait:8.0,cooldown:0.0,hovered:false,next:[0;4]}}
}

impl Expressions {
    pub fn current(&self)->Option<Expression> {self.clip.map(|clip|clip.0)}
    pub fn phase(&self)->Option<(Expression,f32)> {self.clip.map(|(expression,time)|(expression,(time/expression.duration()).clamp(0.0,1.0)))}
    pub fn play(&mut self,expression:Expression) {self.clip=Some((expression,0.0));}
    pub fn play_at(&mut self,expression:Expression,time:f32) {self.clip=Some((expression,time));}
    pub fn clear(&mut self) {self.clip=None;self.wait=8.0;self.cooldown=0.0;self.hovered=false;}
    fn choose(&mut self,bucket:usize,pool:&[Expression])->Expression {
        let expression=pool[self.next[bucket]%pool.len()];self.next[bucket]=self.next[bucket].wrapping_add(1);expression
    }
    pub fn react(&mut self,state:PetState)->f32 {
        use Expression::*;
        let expression=match state {
            PetState::Happy=>self.choose(2,&[Praised,Smug,Delighted,Giggle]),
            PetState::Satisfied=>self.choose(3,&[Heart,Delighted]),
            _=>return 2.6,
        };
        self.play(expression);expression.duration()
    }
    pub fn step(&mut self,state:PetState,dt:f32,visible:bool) {
        use Expression::*;
        if !visible {return;}
        if !matches!(state,PetState::Awake|PetState::Curious|PetState::Happy|PetState::Satisfied) {self.clear();return;}
        self.cooldown=(self.cooldown-dt).max(0.0);
        if let Some((expression,time))=&mut self.clip {
            *time+=dt;
            if *time>=expression.duration() {self.clip=None;self.wait=22.0+(self.next[0]%4) as f32*3.0;}
        }
        let hovered=state==PetState::Curious;
        if hovered && !self.hovered && self.cooldown<=0.0 && self.clip.is_none() {
            let expression=self.choose(1,&[Wink,Blush,SideGlance,ShyPeek]);self.play(expression);self.cooldown=18.0;
        }
        self.hovered=hovered;
        if state==PetState::Awake && self.clip.is_none() && crate::bar_layout::animations_enabled() {
            self.wait-=dt;
            if self.wait<=0.0 {
                let expression=self.choose(0,&[Scan,Delighted,Peek,Smug,Search,Giggle,Praised,ShyPeek]);self.play(expression);
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expressions_follow_real_scenarios_and_system_animation_setting(){
        let mut expressions=Expressions::default();let mut seen=std::collections::HashSet::new();
        let idle_animations=crate::bar_layout::animations_enabled();
        if idle_animations {
            for _ in 0..8 {
                for _ in 0..400 {
                    expressions.step(PetState::Awake,0.1,true);
                    if expressions.current().is_some(){break;}
                }
                seen.insert(expressions.current().expect("日常表情应在等待时间内触发") as usize);
                for _ in 0..100 {
                    expressions.step(PetState::Awake,0.1,true);
                    if expressions.current().is_none(){break;}
                }
                assert!(expressions.current().is_none(),"日常表情应按时结束");
                for _ in 0..210 {expressions.step(PetState::Awake,0.1,true);assert!(expressions.current().is_none(),"日常动作之间应有安静间隔");}
            }
        }else{
            for _ in 0..400 {
                expressions.step(PetState::Awake,0.1,true);
                assert!(expressions.current().is_none(),"系统关闭动画时应保持安静待机");
            }
        }
        for _ in 0..4 {
            expressions.clear();expressions.step(PetState::Curious,0.1,true);seen.insert(expressions.current().unwrap() as usize);
        }
        for state in [PetState::Happy,PetState::Happy,PetState::Happy,PetState::Happy,PetState::Satisfied,PetState::Satisfied] {
            expressions.react(state);seen.insert(expressions.current().unwrap() as usize);
        }
        let expected=Expression::ALL.into_iter().filter(|expression|idle_animations || !matches!(expression,Expression::Scan|Expression::Peek|Expression::Search))
            .map(|expression|expression as usize).collect::<std::collections::HashSet<_>>();
        assert_eq!(seen,expected,"应覆盖当前系统动画设置下可触发的全部表情");
    }
    #[test]
    fn hidden_feedback_pauses_and_hover_has_a_cooldown(){
        let mut expressions=Expressions::default();expressions.react(PetState::Happy);
        expressions.step(PetState::Happy,30.0,false);assert_eq!(expressions.clip.unwrap().1,0.0);
        expressions.step(PetState::Happy,1.0,true);assert_eq!(expressions.clip.unwrap().1,1.0);
        expressions.clear();expressions.step(PetState::Curious,0.1,true);let first=expressions.current().unwrap();
        for _ in 0..50 {expressions.step(PetState::Curious,0.1,true);}
        expressions.step(PetState::Awake,0.1,true);expressions.step(PetState::Curious,0.1,true);
        assert!(expressions.current().is_none(),"快速反复经过不能一直重播互动");
        for _ in 0..180 {expressions.step(PetState::Curious,0.1,true);}
        expressions.step(PetState::Awake,0.1,true);expressions.step(PetState::Curious,0.1,true);
        assert_ne!(expressions.current().unwrap(),first);
    }
    #[test]
    fn focus_music_sleep_and_warning_keep_priority(){
        let mut expressions=Expressions::default();
        for state in [PetState::Focused,PetState::Music,PetState::Worried,PetState::Waking,PetState::Dozing,PetState::Sleeping] {
            expressions.play(Expression::Wink);expressions.step(state,0.1,true);assert!(expressions.current().is_none());
        }
    }
}
