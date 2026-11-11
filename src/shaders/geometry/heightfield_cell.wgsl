// One common finest cell; add heights before composing a quadratic.
alias HFCoefs=array<FInterval,4>;
alias HFPolynomial=array<FInterval,3>;
struct HFPiece {ij:vec2u,height:HFCoefs,mask:HFCoefs};
struct HFRoot {time:FInterval,present:bool,exists:bool};
struct HFRoots {roots:array<HFRoot,2>,coincident:bool,unknown:bool};
struct HFScalars {values:vec2f,count:u32};
fn hf_piece(ij:vec2u)->HFPiece {
  let zero=fi_exact(0.0);
  var result=HFPiece(ij,HFCoefs(zero,zero,zero,zero),HFCoefs(zero,zero,zero,zero));
  let highest=height_data.info.z/height_data.info.x;
  for(var order=0u;order<5u;order++){
    if(order<4u&&height_data.amplitudes[order]==0.0){continue;}
    let factor=highest/hf_repeat(order);let c=hf_coeff(order,ij/vec2u(factor));
    let denominator=fi_exact(f32(factor));
    let x=fi_div(fi_exact(f32(ij.x%factor)),denominator);
    let y=fi_div(fi_exact(f32(ij.y%factor)),denominator);
    var base=c.x;if(order<4u){base-=128.0;}
    let coefficients=HFCoefs(
      fi_add(fi_add(fi_exact(base),fi_scale(x,c.y)),fi_add(fi_scale(y,c.z),fi_scale(fi_mul(x,y),c.w))),
      fi_div(fi_add(fi_exact(c.y),fi_scale(y,c.w)),denominator),
      fi_div(fi_add(fi_exact(c.z),fi_scale(x,c.w)),denominator),
      fi_div(fi_div(fi_exact(c.w),denominator),denominator));
    for(var index=0u;index<4u;index++){
      if(order==4u){result.mask[index]=coefficients[index];}
      else{
        result.height[index]=fi_add(result.height[index],fi_mul(coefficients[index],
          fi_div(fi_exact(height_data.amplitudes[order]),fi_exact(255.0))));
      }
    }
  }
  return result;
}
fn hf_coordinate(piece:HFPiece,axis:u32,o:vec3f,d:vec3f,time:FInterval)->FInterval {
  let scale=0.5*f32(height_data.info.z);
  return fi_add(fi_scale(fi_add(fi_exact(o[axis]),fi_scale(time,d[axis])),scale),
    fi_exact(scale-f32(piece.ij[axis])));
}
fn hf_compose(c:HFCoefs,x:FInterval,dx:FInterval,y:FInterval,dy:FInterval)->HFPolynomial {
  return HFPolynomial(fi_mul(fi_mul(c[3],dx),dy),
    fi_add(fi_add(fi_mul(c[1],dx),fi_mul(c[2],dy)),
      fi_mul(c[3],fi_add(fi_mul(x,dy),fi_mul(y,dx)))),
    fi_add(fi_add(c[0],fi_mul(c[1],x)),fi_add(fi_mul(c[2],y),fi_mul(fi_mul(c[3],x),y))));
}
fn hf_evaluate(p:HFPolynomial,x:FInterval)->FInterval {
  return fi_add(fi_mul(fi_add(fi_mul(p[0],x),p[1]),x),p[2]);
}
fn hf_scalar_roots(p:HFPolynomial)->HFScalars {
  var result=HFScalars(vec2f(0.0),0u);
  let magnitude=max(max(abs(fi_middle(p[0])),abs(fi_middle(p[1]))),abs(fi_middle(p[2])));
  if(magnitude==0.0){return result;}
  let a=fi_middle(p[0])/magnitude;let b=fi_middle(p[1])/magnitude;let c=fi_middle(p[2])/magnitude;
  if(a==0.0){if(b!=0.0){result.values.x=-c/b;result.count=1u;}return result;}
  let discriminant=b*b-4.0*a*c;if(discriminant<0.0){return result;}
  let stable=-0.5*(b+select(-sqrt(discriminant),sqrt(discriminant),b>=0.0));
  if(stable==0.0){result.values.x=-b/(2.0*a);result.count=1u;return result;}
  result.values=vec2f(stable/a,c/stable);result.count=2u;return result;
}
fn hf_root(time:FInterval,exists:bool)->HFRoot {
  return HFRoot(time,time.valid&&time.v.y>=0.0&&time.v.x<=1.0,exists);
}
fn hf_roots(input:HFPolynomial)->HFRoots {
  let empty=HFRoot(fi_invalid(),false,false);
  var result=HFRoots(array<HFRoot,2>(empty,empty),false,false);var p=input;
  for(var index=0u;index<3u;index++){if(!p[index].valid){result.unknown=true;return result;}}
  if(fi_zero(p[0])&&fi_zero(p[1])&&fi_zero(p[2])){result.coincident=true;return result;}
  let unit=FInterval(vec2f(0.0,1.0),true);let value=hf_evaluate(p,unit);
  if(!value.valid){result.unknown=true;return result;}
  if(!fi_contains_zero(value)){return result;}
  if(fi_zero(p[0])&&!fi_contains_zero(p[1])){
    let time=fi_div(fi_neg(p[2]),p[1]);
    result.roots[0]=hf_root(time,true);result.unknown=!time.valid;return result;
  }
  let magnitude=max(max(max(abs(p[0].v.x),abs(p[0].v.y)),max(abs(p[1].v.x),abs(p[1].v.y))),
    max(abs(p[2].v.x),abs(p[2].v.y)));
  for(var index=0u;index<3u;index++){
    p[index]=fi_div(p[index],fi_exact(magnitude));
    if(!p[index].valid){result.unknown=true;return result;}
  }
  if(!fi_contains_zero(p[0])){
    let discriminant=fi_sub(fi_square(p[1]),fi_scale(fi_mul(p[0],p[2]),4.0));
    if(!discriminant.valid){result.unknown=true;return result;}
    if(discriminant.v.y<0.0){return result;}
    if(discriminant.v.x>0.0){
      let square_root=fi_sqrt(discriminant);let denominator=fi_scale(p[0],2.0);
      let a=fi_div(fi_sub(fi_neg(p[1]),square_root),denominator);
      let b=fi_div(fi_add(fi_neg(p[1]),square_root),denominator);
      result.roots=array<HFRoot,2>(hf_root(a,true),hf_root(b,true));
      result.unknown=!a.valid||!b.valid;return result;
    }
  }
  // An analytic proposal and mean-value enclosure; no root-refinement loop.
  let derivative=fi_add(p[1],fi_mul(p[0],FInterval(vec2f(0.0,2.0),true)));
  if(!derivative.valid||fi_contains_zero(derivative)){result.unknown=true;return result;}
  let first=p[2];let last=hf_evaluate(p,fi_exact(1.0));
  if(!last.valid){result.unknown=true;return result;}
  if((first.v.x>0.0&&last.v.x>0.0)||(first.v.y<0.0&&last.v.y<0.0)){return result;}
  var proposal=0.5;let scalar=hf_scalar_roots(p);
  for(var index=0u;index<2u;index++){
    if(index<scalar.count&&scalar.values[index]>=0.0&&scalar.values[index]<=1.0){proposal=scalar.values[index];break;}
  }
  let time=fi_sub(fi_exact(proposal),fi_div(hf_evaluate(p,fi_exact(proposal)),derivative));
  let exists=(first.v.y<0.0&&last.v.x>0.0)||(first.v.x>0.0&&last.v.y<0.0);
  result.roots[0]=hf_root(time,exists);result.unknown=!time.valid;return result;
}
