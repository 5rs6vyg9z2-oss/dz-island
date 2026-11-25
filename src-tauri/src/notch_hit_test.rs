#[derive(Debug)]
pub struct HitBox { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
impl HitBox {
    pub fn from_logical(origin: (i32,i32), scale: f64, rect: (i32,i32,i32,i32)) -> Self {
        let (x,y,w,h)=rect;
        if w<=0 || h<=0 {return Self{left:0,top:0,right:0,bottom:0};}
        Self { left:origin.0+(x as f64*scale).floor() as i32, top:origin.1+(y as f64*scale).floor() as i32,
            right:origin.0+((x+w.max(0)) as f64*scale).ceil() as i32,
            bottom:origin.1+((y+h.max(0)) as f64*scale).ceil() as i32 }
    }
    pub fn contains(&self,x:i32,y:i32)->bool {x>=self.left && x<self.right && y>=self.top && y<self.bottom}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn clock_does_not_capture_surrounding_pixels(){let b=HitBox::from_logical((0,0),1.0,(930,6,60,24));assert!(b.contains(950,15));for p in [(929,15),(990,15),(950,5),(950,30),(900,0)]{assert!(!b.contains(p.0,p.1));}}
    #[test] fn panel_uses_its_actual_vertical_offset(){let b=HitBox::from_logical((0,480),1.0,(10,20,100,40));assert!(b.contains(10,500));assert!(!b.contains(10,480));}
    #[test] fn negative_monitor_coordinates_and_dpi(){let b=HitBox::from_logical((-1920,0),1.5,(600,8,60,24));assert!(b.contains(-1000,20));assert!(!b.contains(-1021,20));assert!(!b.contains(-1000,11));}
    #[test] fn empty_region_is_click_through(){assert!(!HitBox::from_logical((0,0),1.0,(0,0,0,0)).contains(0,0));}
}
