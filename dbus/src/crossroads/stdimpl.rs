use super::crossroads::Crossroads;
use super::handlers::{self, Par, Handlers, MakeHandler, SendMethod, LocalMethod};
use super::info::{IfaceInfo, MethodInfo, PropInfo, Annotations, Argument, Access};
use crate::{arg, Message, Path as PathName};
use super::MethodErr;
use crate::arg::{Arg, Variant, Append, IterAppend};
use std::collections::{HashMap, Bound};
use super::path::{Path, PathData};
use super::context::{MsgCtx, RefCtx};
use std::ffi::CStr;
use crate::strings::{Member, Signature};

pub struct DBusProperties;

fn append_prop<F>(iter: &mut IterAppend, name: &Member, sig: &Signature, f: F) -> Result<(), MethodErr>
where F: FnOnce(&mut IterAppend) -> Result<(), MethodErr> {
    let mut z = None;
    iter.append_dict_entry(|mut iter3| {
        name.append_by_ref(&mut iter3);
        iter3.append_variant(&sig, |iter4| {
            z = Some(f(iter4));
        });
    });
    z.unwrap()
}

fn append_props_ref<H: Handlers, F>(iter: &mut IterAppend, iinfo: &IfaceInfo<H>, mut f: F) -> Result<(), MethodErr>
where F: FnMut(&mut IterAppend, &H::GetProp) -> Result<(), MethodErr> {
    let mut ret = Ok(());
    iter.append_dict(&String::signature(), &Variant::<u8>::signature(), |iter2| {
        for propinfo in iinfo.props.iter() {
            if propinfo.access == Access::Write { continue; }
            if let Some(handler) = propinfo.handlers.0.as_ref() {
                if let Err(e) = append_prop(iter2, &propinfo.name, &propinfo.sig, |ia| { f(ia, handler) }) {
                    ret = Err(e);
                    return;
                }
            }
        }
    });
    ret
}

fn append_props_mut<H: Handlers, F>(iter: &mut IterAppend, iinfo: &mut IfaceInfo<H>, mut f: F) -> Result<(), MethodErr>
where F: FnMut(&mut IterAppend, &mut H::GetProp) -> Result<(), MethodErr> {
    let mut ret = Ok(());
    iter.append_dict(&String::signature(), &Variant::<u8>::signature(), |iter2| {
        for propinfo in iinfo.props.iter_mut() {
            if propinfo.access == Access::Write { continue; }
            if let Some(handler) = propinfo.handlers.0.as_mut() {
                if let Err(e) = append_prop(iter2, &propinfo.name, &propinfo.sig, |ia| { f(ia, handler) }) {
                    ret = Err(e);
                    return;
                }
            }
        }
    });
    ret
}

fn setprop_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&mut H::SetProp, &mut Path<H>, &mut arg::Iter, &mut MsgCtx) -> Result<bool, MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname) = (iter.read()?, iter.read()?);
    let path = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let (propinfo, pathdata) = cr.prop_lookup_mut(path.as_cstr(), iname, propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;
    if propinfo.access == Access::Read { Err(MethodErr::no_property(&"Property is read only"))? };
    let handler = propinfo.handlers.1.as_mut()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not written to") })?;

    // Now descend into the variant.
    use arg::Arg;
    let mut subiter = iter.recurse(Variant::<bool>::ARG_TYPE).ok_or_else(|| MethodErr::invalid_arg(&2))?;
    if *subiter.signature() != *propinfo.sig {
        Err(MethodErr::failed(&format!("Property {} cannot change type", propinfo.name)))?;
    }
    if f(handler, pathdata, &mut subiter, ctx)? {
        unimplemented!("Emits signal here");
    }
    Ok(ctx.message.method_return())
}

fn setprop_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&H::SetProp, &mut arg::Iter, &mut MsgCtx, &RefCtx<H>) -> Result<bool, MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname): (&CStr, &CStr) = (iter.read()?, iter.read()?);
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let propinfo = refctx.iinfo.props.iter().find(|x| x.name.as_cstr() == propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;

    if propinfo.access == Access::Read { Err(MethodErr::no_property(&"Property is read only"))? };
    let handler = propinfo.handlers.1.as_ref()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not written to") })?;

    // Now descend into the variant.
    use arg::Arg;
    let mut subiter = iter.recurse(Variant::<bool>::ARG_TYPE).ok_or_else(|| MethodErr::invalid_arg(&2))?;
    if *subiter.signature() != *propinfo.sig {
        Err(MethodErr::failed(&format!("Property {} cannot change type", propinfo.name)))?;
    }
    if f(handler, &mut subiter, ctx, &refctx)? {
        unimplemented!("Emits signal here");
    }
    Ok(ctx.message.method_return())
}


fn getprop_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&mut H::GetProp, &mut Path<H>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname) = (iter.read()?, iter.read()?);
    let path = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let (propinfo, pathdata) = cr.prop_lookup_mut(path.as_cstr(), iname, propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;
    if propinfo.access == Access::Write { Err(MethodErr::no_property(&"Property is write only"))? };
    let handler = propinfo.handlers.0.as_mut()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not be read from") })?;

    let mut mret = ctx.message.method_return();
    {
        let mut iter = arg::IterAppend::new(&mut mret);
        let mut z = None;
        iter.append_variant(&propinfo.sig, |subi| {
            z = Some(f(handler, pathdata, subi, ctx));
        });
        z.unwrap()?;
    }
    Ok(mret)
}

fn getprop_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&H::GetProp, &mut arg::IterAppend, &mut MsgCtx, &RefCtx<H>) -> Result<(), MethodErr> {
    let mut iter = ctx.message.iter_init();
    let (iname, propname): (&CStr, &CStr) = (iter.read()?, iter.read()?);
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let propinfo = refctx.iinfo.props.iter().find(|x| x.name.as_cstr() == propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;

    if propinfo.access == Access::Write { Err(MethodErr::no_property(&"Property is write only"))? };
    let handler = propinfo.handlers.0.as_ref()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not read from") })?;

    let mut mret = ctx.message.method_return();
    {
        let mut iter = arg::IterAppend::new(&mut mret);
        let mut z = None;
        iter.append_variant(&propinfo.sig, |subi| {
            z = Some(f(handler, subi, ctx, &refctx));
        });
        z.unwrap()?;
    }
    Ok(mret)
}

fn getallprops_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, mut f: F) -> Result<Message, MethodErr>
where F: FnMut(&H::GetProp, &mut arg::IterAppend, &mut MsgCtx, &RefCtx<H>) -> Result<(), MethodErr> {
    let mut iter = ctx.message.iter_init();
    let iname: &CStr = iter.read()?;
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;

    let mut mret = ctx.message.method_return();
    {
        append_props_ref(&mut arg::IterAppend::new(&mut mret), &refctx.iinfo, |iter4, handler| {
            f(handler, iter4, ctx, &refctx)
        })?;
    }
    Ok(mret)
}

fn objmgr_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, mut f: F) -> Result<Message, MethodErr>
where F: FnMut(&mut H::GetProp, &mut Path<H>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr>
{
    let pathname = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let mut p = Vec::<u8>::from(pathname.as_bytes());
    if !p.ends_with(b"/") { p.push(b'/'); }

    let mut children = cr.paths.range_mut::<CStr,_>((Bound::Included(pathname.as_cstr()), Bound::Unbounded));
    let cr_reg = &mut cr.reg;

    let mut ret = Ok(());
    let mut mret = ctx.message.method_return();
    {
        let mut ia = arg::IterAppend::new(&mut mret);
        type VArg1 = HashMap<String, Variant<u8>>;
        type VArg2 = HashMap<String, VArg1>;
        ia.append_dict(&PathName::signature(), &VArg2::signature(), |ia2| {
            while let Some((c, pdata)) = children.next() {
                if !c.as_bytes().starts_with(&p) { break; }
                ia2.append_dict_entry(|mut ia3| {
                    pdata.name().append_by_ref(&mut ia3);
                    ia3.append_dict(&String::signature(), &VArg1::signature(), |mut ia4| {
                        for entry in cr_reg.values_mut() {
                            if !pdata.get_from_typeid(entry.typeid).is_none() { continue };
                            entry.info.name.append_by_ref(&mut ia4);
                            if let Err(e) = append_props_mut(&mut ia4, &mut entry.info, |ia5, handler| {
                                f(handler, pdata, ia5, ctx)
                            }) { ret = Err(e); return };
                        }
                    });
                    if ret.is_err() { return; }
                });
                if ret.is_err() { return; }
            }
        });
    }
    ret.map(|_| mret)
}

impl DBusProperties {
    fn register_custom<H: Handlers>(cr: &mut Crossroads<H>, get: H::Method, getall: H::Method, set: H::Method) where Self: PathData<H::Iface> {
        cr.register::<Self,_>("org.freedesktop.DBus.Properties")
            .method_custom::<(String, String), (Variant<u8>,)>("Get".into(), ("interface_name", "property_name"), ("value",), get)
            .method_custom::<(String,), (HashMap<String, Variant<u8>>,)>("GetAll".into(), ("interface_name",), ("props",), getall)
            .method_custom::<(String, String, Variant<u8>), ()>("Set".into(), ("interface_name", "property_name", "value"), (), set)
            .on_path_insert(|p, cr| {
                if cr.reg.values().any(|entry| !entry.info.props.is_empty() && p.get_from_typeid(entry.typeid).is_some()) {
                    p.insert(DBusProperties)
                }
            });
    }

    pub fn register_par(cr: &mut Crossroads<Par>) {
        Self::register_custom(cr,
            Box::new(|ctx, refctx| {
                Some(getprop_ref(ctx, refctx, |h, i, ctx, refctx| h(i, ctx, refctx)).unwrap_or_else(|e| e.to_message(ctx.message)))
            }),
            Box::new(|ctx, refctx| {
                Some(getallprops_ref(ctx, refctx, |h, i, ctx, refctx| h(i, ctx, refctx)).unwrap_or_else(|e| e.to_message(ctx.message)))
            }),
            Box::new(|ctx, refctx| {
                Some(setprop_ref(ctx, refctx, |h, i, ctx, refctx| h(i, ctx, refctx)).unwrap_or_else(|e| e.to_message(ctx.message)))
            })
        );
    }

    pub fn register_local(cr: &mut Crossroads<handlers::Local>) {
//        Self::register(cr, unimplemented!(), unimplemented!(), unimplemented!());
    }

    pub fn register(cr: &mut Crossroads<()>) {
        let getprop = |cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            getprop_mut(cr, ctx, |f, path, ia, ctx| { f(path, ia, ctx) })
        };
        let getallprop = |cr: &mut Crossroads<()>, ctx: &mut MsgCtx| -> Option<Message> { unimplemented!() };
        let setprop = |cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            setprop_mut(cr, ctx, |f, path, iter, ctx| { f(path, iter, ctx) })
        };

        // let x = MakeHandler::<<() as Handlers>::Method, ((), (), (), DBusProperties), (i8, ())>::make(x);
        Self::register_custom(cr,
            MakeHandler::make(getprop), MakeHandler::make(getallprop), MakeHandler::make(setprop)
        );
//        Self::register(cr, unimplemented!(), unimplemented!(), unimplemented!());
    }

}

pub struct DBusObjectManager;

impl DBusObjectManager {
    fn register_custom<H: Handlers>(cr: &mut Crossroads<H>, m: H::Method) where Self: PathData<H::Iface> {
        type OutArg = HashMap<PathName<'static>, HashMap<String, HashMap<String, Variant<u8>>>>;
        cr.register::<Self,_>("org.freedesktop.DBus.ObjectManager")
            .method_custom::<(), (OutArg,)>("GetManagedObjects".into(), (), ("objpath_interfaces_and_properties",), m);
    }

    pub fn register(cr: &mut Crossroads<()>) {
        Self::register_custom(cr, MakeHandler::make(|cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            objmgr_mut(cr, ctx, |h, path, ia, ctx| h(path, ia, ctx))
        }))
    }
}


pub struct DBusIntrospectable;

fn introspect_anns(anns: &Annotations, prefix: &str) -> String {
    let mut r = String::new();
    for (k, v) in anns.iter() {
        r += &format!("{}<annotation name=\"{}\" value=\"{}\"/>\n", prefix, k, v);
    }
    r
}

fn introspect_args(args: &[Argument], dir: Option<&str>, prefix: &str) -> String {
    let mut r = String::new();
    for a in args {
        r += &format!("{}<arg name=\"{}\" type=\"{}\"", prefix, a.name, a.sig);
        if let Some(dir) = dir { r += &format!(" direction=\"{}\"", dir); }
        if a.anns.len() == 0 {
            r += "/>\n";
        } else {
            let inner_prefix = format!("{}  ", prefix);
            r += &format!(">\n{}{}</arg>\n", introspect_anns(&a.anns, &inner_prefix), prefix);
        }
    }
    r
}

fn introspect_iface<H: Handlers>(iface: &IfaceInfo<H>) -> String {
    let mut r = format!("  <interface name=\"{}\">\n", iface.name);
    for x in &iface.methods {
        r += &format!("    <method name=\"{}\">\n", x.name);
        r += &introspect_args(&x.i_args, Some("in"), "      ");
        r += &introspect_args(&x.o_args, Some("out"), "      ");
        r += &introspect_anns(&x.anns, "      ");
        r += "    </method>\n";
    }

    for x in &iface.signals {
        r += &format!("    <signal name=\"{}\">\n", x.name);
        r += &introspect_args(&x.args, None, "      ");
        r += &introspect_anns(&x.anns, "      ");
        r += "    </signal>\n";
    }

    for x in &iface.props {
        use super::info::Access;
        let a = match x.access {
            Access::Read => "read",
            Access::Write => "write",
            Access::ReadWrite => "readwrite",
        };
        r += &format!("    <property name=\"{}\" type=\"{}\" access=\"{}\"", x.name, x.sig, a);
        if x.anns.len() == 0 {
            r += "/>\n";
        } else {
            r += &format!(">\n{}    </property>\n", introspect_anns(&x.anns, "      "));
        }
    }

    r += &introspect_anns(&iface.anns, "    ");
    r += "  </interface>\n";
    r
}

fn introspect<H: Handlers>(cr: &Crossroads<H>, path: &Path<H>) -> String {
    use std::ffi::{CStr, CString};
    let name = path.name();
    let mut p = Vec::<u8>::from(name.as_bytes());
    if !p.ends_with(b"/") { p.push(b'/'); }

    let mut children = cr.paths.range::<CStr,_>((Bound::Excluded(name.as_cstr()), Bound::Unbounded));
    let mut childstr = String::new();
    while let Some((c, _)) = children.next() {
        if !c.as_bytes().starts_with(&p) { break; }
        let csub: &str = &c.to_str().unwrap()[p.len()..];
        childstr = format!("{}  <node name=\"{}\"/>\n", childstr, csub);
    }

    let mut ifacestr = String::new();
    for entry in cr.reg.values() {
        if path.get_from_typeid(entry.typeid).is_some() {
            ifacestr += &introspect_iface(&entry.info);
        }
    }

    let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, name, ifacestr, childstr);
    nodestr
}

impl DBusIntrospectable {
    pub fn register<H: Handlers>(cr: &mut Crossroads<H>) where Self: PathData<H::Iface> {
        cr.register::<Self,_>("org.freedesktop.DBus.Introspectable")
            .method("Introspect", (), ("xml_data",), |_: &mut MsgCtx, c: &RefCtx<H>, _: ()| {
                Ok((introspect(c.crossroads, c.path),))
            })
            .on_path_insert(|p, cr| p.insert(DBusIntrospectable));
    }
}
