<!-- Tufts VUE 3.3.0 concept-map (bdd-types.vue) 2020-05-25 -->
<!-- Tufts VUE: http://vue.tufts.edu/ -->
<!-- Do Not Remove: VUE mapping @version(1.1) jar:file:/C:/Program%20Files%20(x86)/VUE/VUE.jar!/tufts/vue/resources/lw_mapping_1_1.xml -->
<!-- Do Not Remove: Saved date Mon May 25 11:35:48 EDT 2020 by tange on platform Windows 8 6.2 in JVM 1.7.0_21-b11 -->
<!-- Do Not Remove: Saving version @(#)VUE: built October 8 2015 at 1658 by tomadm on Linux 2.6.32-504.23.4.el6.x86_64 i386 JVM 1.7.0_21-b11(bits=32) -->
<?xml version="1.0" encoding="US-ASCII"?>
<LW-MAP xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xsi:noNamespaceSchemaLocation="none" ID="0" label="bdd-types.vue"
    created="1590418643801" x="0.0" y="0.0" width="1.4E-45"
    height="1.4E-45" strokeWidth="0.0" autoSized="false">
    <resource referenceCreated="1590420948449" size="30806"
        spec="D:\ver\bex\doc\bdd-types.vue" type="1" xsi:type="URLResource">
        <title>bdd-types.vue</title>
        <property key="File" value="D:\ver\bex\doc\bdd-types.vue"/>
    </resource>
    <fillColor>#FFFFFF</fillColor>
    <strokeColor>#404040</strokeColor>
    <textColor>#000000</textColor>
    <font>SansSerif-plain-14</font>
    <URIString>http://vue.tufts.edu/rdf/resource/4c5d81a8c0a800750116aba6dc8d68cc</URIString>
    <child ID="6" label="pub struct BDDNode{ v, hi, lo }" layerID="1"
        created="1590418669513" x="337.0" y="129.0" width="321.0"
        height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81a9c0a800750116aba6e8c8f58e</URIString>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="7" label="pub struct ITE {i:NID, t:NID, e:NID}"
        layerID="1" created="1590418734930" x="331.0" y="172.0"
        width="371.0" height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81aac0a800750116aba66d79acd4</URIString>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="8" label="pub trait BddState" layerID="1"
        created="1590418763188" x="742.0" y="38.0" width="565.5"
        height="262.5" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#F4E5FF</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81aac0a800750116aba6f8e4895a</URIString>
        <child ID="11" label="new()" created="1590418837113" x="34.0"
            y="30.0" width="61.0" height="30.0" strokeWidth="1.0"
            autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81abc0a800750116aba6483bad4f</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="13" label="fn tup(&amp;self, n:NID)-> (NID, NID)"
            created="1590418850491" x="34.0" y="55.5" width="341.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81abc0a800750116aba66d92cc49</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="15"
            label="fn simple_node(&amp;mut self, v:VID, hilo:HILO)->NID "
            created="1590418875898" x="34.0" y="81.0" width="501.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81adc0a800750116aba63f8dadb1</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="16" label="fn nvars(&amp;self)->usize;"
            created="1590418890817" x="34.0" y="106.5" width="241.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81aec0a800750116aba64226e67d</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="18" label="fn get_hilo(&amp;self, n:NID)->HILO;"
            created="1590418901394" x="34.0" y="132.0" width="331.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81aec0a800750116aba654d37a46</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="20"
            label="fn get_memo&lt;'a>(&amp;'a self, v:VID, ite:&amp;ITE) -> Option&lt;&amp;'a NID>"
            created="1590418938827" x="34.0" y="157.5" width="621.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81afc0a800750116aba65aac6700</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="22"
            label="fn put_xmemo(&amp;mut self, ite:ITE, new_nid:NID);"
            created="1590418947420" x="34.0" y="183.0" width="471.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b0c0a800750116aba6933a0880</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="24"
            label="fn get_simple_node&lt;'a>(&amp;'a self, v:VID, hilo:HILO)-> Option&lt;&amp;'a NID>;"
            created="1590418957000" x="34.0" y="208.5" width="701.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b0c0a800750116aba6d60f1155</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <child ID="25"
            label="fn put_simple_node(&amp;mut self, v:VID, hilo:HILO)->NID"
            created="1590418961906" x="34.0" y="234.0" width="531.0"
            height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
            <fillColor>#00000000</fillColor>
            <strokeColor>#00000000</strokeColor>
            <textColor>#000000</textColor>
            <font>Consolas-plain-18</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b1c0a800750116aba69815bfd8</URIString>
            <shape xsi:type="rectangle"/>
        </child>
        <shape arcwidth="20.0" archeight="20.0" xsi:type="roundRect"/>
    </child>
    <child ID="9" label="pub struct SafeVarKeyedBddState" layerID="1"
        created="1590418774687" x="330.0" y="213.0" width="321.0"
        height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b2c0a800750116aba65f8a5dcd</URIString>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="10" label="pub struct UnsafeVarKeyedBddState {"
        layerID="1" created="1590418782898" x="332.0" y="259.0"
        width="361.0" height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b3c0a800750116aba636337732</URIString>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="26"
        label="pub trait BddWorker&lt;S:BddState> : Sized + Serialize {"
        layerID="1" created="1590418997915" x="682.0" y="355.0"
        width="541.0" height="177.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#F4E5FF</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b8c0a800750116aba686795779</URIString>
        <child ID="27"
            label="fn new(nvars:usize)->Self;&#xa;    &#xa;    &#xa;        fn new_with_state(state: S)->Self;&#xa;    &#xa;    &#xa;        fn nvars(&amp;self)->usize;&#xa;    &#xa;    &#xa;        fn tup(&amp;self, n:NID)->(NID,NID);&#xa;    &#xa;    &#xa;        fn ite(&amp;mut self, f:NID, g:NID, h:NID)->NID;&#xa;    &#xa;    &#xa;        fn get_state()->S;"
            created="1590419012191" x="5.0" y="30.0" width="488.0"
            height="141.0" strokeWidth="0.0" autoSized="false" xsi:type="text">
            <strokeColor>#404040</strokeColor>
            <textColor>#000000</textColor>
            <font>SansSerif-plain-14</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81b9c0a800750116aba61e7a7a85</URIString>
            <richText>&lt;html&gt;
  &lt;head style="color: #000000" color="#000000"&gt;
    &lt;style type="text/css"&gt;
      &lt;!--
        body { margin-bottom: 0px; margin-right: 0px; font-family: Consolas; color: #000000; font-size: 18; margin-left: 0px; margin-top: 0px }
        ol { vertical-align: middle; font-family: Consolas; margin-left: 30; font-size: 18; margin-top: 6; list-style-position: outside }
        p { margin-bottom: 0; margin-right: 0; color: #000000; margin-left: 0; margin-top: 0 }
        ul { vertical-align: middle; font-family: Consolas; margin-left: 30; font-size: 18; margin-top: 6; list-style-position: outside }
      --&gt;
    &lt;/style&gt;
    
  &lt;/head&gt;
  &lt;body&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn new(nvars:usize)-&amp;gt;Self;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn new_with_state(state: S)-&amp;gt;Self;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn nvars(&amp;amp;self)-&amp;gt;usize;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn tup(&amp;amp;self, n:NID)-&amp;gt;(NID,NID);
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn ite(&amp;amp;mut self, f:NID, g:NID, h:NID)-&amp;gt;NID;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn get_state()-&amp;gt;S;
    &lt;/p&gt;
  &lt;/body&gt;
&lt;/html&gt;
</richText>
            <label>fn new(nvars:usize)-&gt;Self;
    
    
        fn new_with_state(state: S)-&gt;Self;
    
    
        fn nvars(&amp;self)-&gt;usize;
    
    
        fn tup(&amp;self, n:NID)-&gt;(NID,NID);
    
    
        fn ite(&amp;mut self, f:NID, g:NID, h:NID)-&gt;NID;
    
    
        fn get_state()-&gt;S;</label>
        </child>
        <shape arcwidth="20.0" archeight="20.0" xsi:type="roundRect"/>
    </child>
    <child ID="28"
        label="pub struct SimpleBddWorker&lt;S:BddState> { state:S }"
        layerID="1" created="1590419051905" x="406.0" y="571.0"
        width="518.0" height="62.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bac0a800750116aba60ae7005d</URIString>
        <child ID="31"
            label="#[inline] fn ite_norm(&amp;mut self, ite:ITE)->NID {"
            created="1590419112982" x="5.0" y="30.0" width="508.0"
            height="26.0" strokeWidth="0.0" autoSized="false" xsi:type="text">
            <strokeColor>#404040</strokeColor>
            <textColor>#000000</textColor>
            <font>SansSerif-plain-14</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c6371f5c0a800750116aba67443f620</URIString>
            <richText>&lt;html&gt;
  &lt;head style="color: #000000" color="#000000"&gt;
    &lt;style type="text/css"&gt;
      &lt;!--
        body { margin-bottom: 0px; margin-right: 0px; font-family: Consolas; color: #000000; font-size: 18; margin-left: 0px; margin-top: 0px }
        ol { vertical-align: middle; font-family: Consolas; margin-left: 30; font-size: 18; margin-top: 6; list-style-position: outside }
        p { margin-bottom: 0; margin-right: 0; color: #000000; margin-left: 0; margin-top: 0 }
        ul { vertical-align: middle; font-family: Consolas; margin-left: 30; font-size: 18; margin-top: 6; list-style-position: outside }
      --&gt;
    &lt;/style&gt;
    
  &lt;/head&gt;
  &lt;body&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        #[inline] fn ite_norm(&amp;amp;mut self, ite:ITE)-&amp;gt;NID {
    &lt;/p&gt;
  &lt;/body&gt;
&lt;/html&gt;
</richText>
            <label>#[inline] fn ite_norm(&amp;mut self, ite:ITE)-&gt;NID {</label>
        </child>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="30" layerID="1" created="1590419070409" x="720.73035"
        y="531.5" width="71.74133" height="40.0" strokeWidth="1.0"
        autoSized="false" controlCount="0" arrowState="2" xsi:type="link">
        <strokeColor>#404040</strokeColor>
        <textColor>#404040</textColor>
        <font>Arial-plain-11</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bac0a800750116aba683f786ce</URIString>
        <point1 x="721.23035" y="571.0"/>
        <point2 x="791.9717" y="532.0"/>
        <ID1 xsi:type="node">28</ID1>
        <ID2 xsi:type="node">26</ID2>
    </child>
    <child ID="32"
        label="pub struct BddSwarm &lt;S:BddState+'static>&#x9;"
        layerID="1" created="1590419115094" x="978.0" y="580.0"
        width="421.0" height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c6371f5c0a800750116aba6411e6d38</URIString>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="33" layerID="1" created="1590419192150" x="1089.8615"
        y="531.5" width="75.77222" height="49.0" strokeWidth="1.0"
        autoSized="false" controlCount="0" arrowState="2" xsi:type="link">
        <strokeColor>#404040</strokeColor>
        <textColor>#404040</textColor>
        <font>Arial-plain-11</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c6371f6c0a800750116aba635d60d5d</URIString>
        <point1 x="1165.1337" y="580.0"/>
        <point2 x="1090.3615" y="532.0"/>
        <ID1 xsi:type="node">32</ID1>
        <ID2 xsi:type="node">26</ID2>
    </child>
    <child ID="34"
        label="pub struct BddBase&lt;S:BddState, W:BddWorker&lt;S>>"
        layerID="1" created="1590419277108" x="1511.0" y="941.0"
        width="471.0" height="30.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#FEFEC9</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c6371f6c0a800750116aba6ac4f6f78</URIString>
        <shape xsi:type="rectangle"/>
    </child>
    <child ID="35" label="trait base::Base" layerID="1"
        created="1590419283801" x="1395.0" y="62.0" width="788.0"
        height="835.0" strokeWidth="1.0" autoSized="true" xsi:type="node">
        <fillColor>#F4E5FF</fillColor>
        <strokeColor>#000000</strokeColor>
        <textColor>#000000</textColor>
        <font>Consolas-plain-18</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c6371f7c0a800750116aba6271d6a91</URIString>
        <child ID="37"
            label="/// Node identifier type. Usually mapped to xxx::NID&#xa;    &#xa;    &#xa;        type N;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        /// Variable identifier type. Usually mapped to xxx::VID&#xa;    &#xa;    &#xa;        type V;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        fn new(n:usize)->Self where Self:Sized; // Sized so we can use trait &#xa;      objects.&#xa;    &#xa;    &#xa;        fn num_vars(&amp;self)->usize;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        fn o(&amp;self)->Self::N;&#xa;    &#xa;    &#xa;        fn i(&amp;self)->Self::N;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        fn var(&amp;mut self, i:Self::V)->Self::N;&#xa;    &#xa;    &#xa;        fn when_hi(&amp;mut self, v:Self::V, n:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        fn when_lo(&amp;mut self, v:Self::V, n:Self::N)->Self::N;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        fn not(&amp;mut self, x:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        fn and(&amp;mut self, x:Self::N, y:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        fn xor(&amp;mut self, x:Self::N, y:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        fn or(&amp;mut self, x:Self::N, y:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        #[cfg(todo)] fn mj(&amp;mut self, x:Self::N, y:Self::N, &#xa;      z:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        #[cfg(todo)] fn ch(&amp;mut self, x:Self::N, y:Self::N, &#xa;      z:Self::N)->Self::N;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        fn def(&amp;mut self, s:String, i:u32)->Self::N;&#xa;    &#xa;    &#xa;        fn tag(&amp;mut self, n:Self::N, s:String)->Self::N;&#xa;    &#xa;    &#xa;        fn get(&amp;mut self, _s:&amp;String)->Option&lt;Self::N>;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        /// substitute node for variable in context.&#xa;    &#xa;    &#xa;        fn sub(&amp;mut self, v:Self::V, n:Self::N, ctx:Self::N)->Self::N;&#xa;    &#xa;    &#xa;        fn solutions(&amp;self)->&amp;dyn Iterator&lt;Item=Vec&lt;bool>>;&#xa;    &#xa;    &#xa;      &#xa;    &#xa;    &#xa;        fn save(&amp;self, path:&amp;str)->::std::io::Result&lt;()>;&#xa;    &#xa;    &#xa;        fn save_dot(&amp;self, n:Self::N, path:&amp;str);&#xa;    &#xa;    &#xa;        fn show_named(&amp;self, n:Self::N, path:&amp;str);"
            created="1590419507903" x="5.0" y="30.0" width="778.0"
            height="799.0" strokeWidth="0.0" autoSized="false" xsi:type="text">
            <strokeColor>#404040</strokeColor>
            <textColor>#000000</textColor>
            <font>SansSerif-plain-14</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c649c37c0a800750116aba623a62fd4</URIString>
            <richText>&lt;html&gt;
  &lt;head style="color: #000000" color="#000000"&gt;
    &lt;style type="text/css"&gt;
      &lt;!--
        body { margin-bottom: 0px; margin-right: 0px; font-family: Consolas; color: #000000; font-size: 18; margin-left: 0px; margin-top: 0px }
        ol { vertical-align: middle; font-family: Consolas; margin-left: 30; font-size: 18; margin-top: 6; list-style-position: outside }
        p { margin-bottom: 0; margin-right: 0; color: #000000; margin-left: 0; margin-top: 0 }
        ul { vertical-align: middle; font-family: Consolas; margin-left: 30; font-size: 18; margin-top: 6; list-style-position: outside }
      --&gt;
    &lt;/style&gt;
    
  &lt;/head&gt;
  &lt;body&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        /// Node identifier type. Usually mapped to xxx::NID
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        type N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        /// Variable identifier type. Usually mapped to xxx::VID
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        type V;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn new(n:usize)-&amp;gt;Self where Self:Sized; // Sized so we can use trait 
      objects.
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn num_vars(&amp;amp;self)-&amp;gt;usize;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn o(&amp;amp;self)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn i(&amp;amp;self)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn var(&amp;amp;mut self, i:Self::V)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn when_hi(&amp;amp;mut self, v:Self::V, n:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn when_lo(&amp;amp;mut self, v:Self::V, n:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn not(&amp;amp;mut self, x:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn and(&amp;amp;mut self, x:Self::N, y:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn xor(&amp;amp;mut self, x:Self::N, y:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn or(&amp;amp;mut self, x:Self::N, y:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        #[cfg(todo)] fn mj(&amp;amp;mut self, x:Self::N, y:Self::N, 
      z:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        #[cfg(todo)] fn ch(&amp;amp;mut self, x:Self::N, y:Self::N, 
      z:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn def(&amp;amp;mut self, s:String, i:u32)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn tag(&amp;amp;mut self, n:Self::N, s:String)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn get(&amp;amp;mut self, _s:&amp;amp;String)-&amp;gt;Option&amp;lt;Self::N&amp;gt;;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        /// substitute node for variable in context.
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn sub(&amp;amp;mut self, v:Self::V, n:Self::N, ctx:Self::N)-&amp;gt;Self::N;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn solutions(&amp;amp;self)-&amp;gt;&amp;amp;dyn Iterator&amp;lt;Item=Vec&amp;lt;bool&amp;gt;&amp;gt;;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn save(&amp;amp;self, path:&amp;amp;str)-&amp;gt;::std::io::Result&amp;lt;()&amp;gt;;
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn save_dot(&amp;amp;self, n:Self::N, path:&amp;amp;str);
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
        fn show_named(&amp;amp;self, n:Self::N, path:&amp;amp;str);
    &lt;/p&gt;
    &lt;p style="color: #000000" color="#000000"&gt;
      
    &lt;/p&gt;
  &lt;/body&gt;
&lt;/html&gt;
</richText>
            <label>/// Node identifier type. Usually mapped to xxx::NID
    
    
        type N;
    
    
      
    
    
        /// Variable identifier type. Usually mapped to xxx::VID
    
    
        type V;
    
    
      
    
    
        fn new(n:usize)-&gt;Self where Self:Sized; // Sized so we can use trait 
      objects.
    
    
        fn num_vars(&amp;self)-&gt;usize;
    
    
      
    
    
        fn o(&amp;self)-&gt;Self::N;
    
    
        fn i(&amp;self)-&gt;Self::N;
    
    
      
    
    
        fn var(&amp;mut self, i:Self::V)-&gt;Self::N;
    
    
        fn when_hi(&amp;mut self, v:Self::V, n:Self::N)-&gt;Self::N;
    
    
        fn when_lo(&amp;mut self, v:Self::V, n:Self::N)-&gt;Self::N;
    
    
      
    
    
        fn not(&amp;mut self, x:Self::N)-&gt;Self::N;
    
    
        fn and(&amp;mut self, x:Self::N, y:Self::N)-&gt;Self::N;
    
    
        fn xor(&amp;mut self, x:Self::N, y:Self::N)-&gt;Self::N;
    
    
        fn or(&amp;mut self, x:Self::N, y:Self::N)-&gt;Self::N;
    
    
        #[cfg(todo)] fn mj(&amp;mut self, x:Self::N, y:Self::N, 
      z:Self::N)-&gt;Self::N;
    
    
        #[cfg(todo)] fn ch(&amp;mut self, x:Self::N, y:Self::N, 
      z:Self::N)-&gt;Self::N;
    
    
      
    
    
        fn def(&amp;mut self, s:String, i:u32)-&gt;Self::N;
    
    
        fn tag(&amp;mut self, n:Self::N, s:String)-&gt;Self::N;
    
    
        fn get(&amp;mut self, _s:&amp;String)-&gt;Option&lt;Self::N&gt;;
    
    
      
    
    
        /// substitute node for variable in context.
    
    
        fn sub(&amp;mut self, v:Self::V, n:Self::N, ctx:Self::N)-&gt;Self::N;
    
    
        fn solutions(&amp;self)-&gt;&amp;dyn Iterator&lt;Item=Vec&lt;bool&gt;&gt;;
    
    
      
    
    
        fn save(&amp;self, path:&amp;str)-&gt;::std::io::Result&lt;()&gt;;
    
    
        fn save_dot(&amp;self, n:Self::N, path:&amp;str);
    
    
        fn show_named(&amp;self, n:Self::N, path:&amp;str);</label>
        </child>
        <shape arcwidth="20.0" archeight="20.0" xsi:type="roundRect"/>
    </child>
    <child ID="36" layerID="1" created="1590419307301" x="1747.3379"
        y="896.5" width="4.9244385" height="45.0" strokeWidth="1.0"
        autoSized="false" controlCount="0" arrowState="2" xsi:type="link">
        <strokeColor>#404040</strokeColor>
        <textColor>#404040</textColor>
        <font>Arial-plain-11</font>
        <URIString>http://vue.tufts.edu/rdf/resource/4c6371f7c0a800750116aba616f86db6</URIString>
        <point1 x="1747.8378" y="941.0"/>
        <point2 x="1751.7622" y="897.0"/>
        <ID1 xsi:type="node">34</ID1>
        <ID2 xsi:type="node">35</ID2>
    </child>
    <layer ID="1" label="Layer 1" created="1590418643804" x="0.0"
        y="0.0" width="1.4E-45" height="1.4E-45" strokeWidth="0.0" autoSized="false">
        <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bbc0a800750116aba6d2fedf55</URIString>
    </layer>
    <userZoom>0.75</userZoom>
    <userOrigin x="148.25" y="15.125001"/>
    <presentationBackground>#202020</presentationBackground>
    <PathwayList currentPathway="0" revealerIndex="-1">
        <pathway ID="0" label="Untitled Pathway" created="1590418643800"
            x="0.0" y="0.0" width="1.4E-45" height="1.4E-45"
            strokeWidth="0.0" autoSized="false" currentIndex="-1" open="true">
            <strokeColor>#B3993333</strokeColor>
            <textColor>#000000</textColor>
            <font>SansSerif-plain-14</font>
            <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bbc0a800750116aba689bd94f6</URIString>
            <masterSlide ID="2" created="1590418643868" x="0.0" y="0.0"
                width="800.0" height="600.0" locked="true"
                strokeWidth="0.0" autoSized="false">
                <fillColor>#000000</fillColor>
                <strokeColor>#404040</strokeColor>
                <textColor>#000000</textColor>
                <font>SansSerif-plain-14</font>
                <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bbc0a800750116aba69fbbca65</URIString>
                <titleStyle ID="3" label="Header"
                    created="1590418644238" x="335.5" y="172.5"
                    width="129.0" height="55.0" strokeWidth="0.0"
                    autoSized="true" isStyle="true" xsi:type="node">
                    <strokeColor>#404040</strokeColor>
                    <textColor>#FFFFFF</textColor>
                    <font>Gill Sans-plain-36</font>
                    <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bcc0a800750116aba68c7e167b</URIString>
                    <shape xsi:type="rectangle"/>
                </titleStyle>
                <textStyle ID="4" label="Slide Text"
                    created="1590418644242" x="346.5" y="281.5"
                    width="107.0" height="37.0" strokeWidth="0.0"
                    autoSized="true" isStyle="true" xsi:type="node">
                    <strokeColor>#404040</strokeColor>
                    <textColor>#FFFFFF</textColor>
                    <font>Gill Sans-plain-22</font>
                    <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bcc0a800750116aba60229668c</URIString>
                    <shape xsi:type="rectangle"/>
                </textStyle>
                <linkStyle ID="5" label="Links" created="1590418644244"
                    x="373.5" y="384.0" width="53.0" height="32.0"
                    strokeWidth="0.0" autoSized="true" isStyle="true" xsi:type="node">
                    <strokeColor>#404040</strokeColor>
                    <textColor>#B3BFE3</textColor>
                    <font>Gill Sans-plain-18</font>
                    <URIString>http://vue.tufts.edu/rdf/resource/4c5d81bcc0a800750116aba6e2833fb7</URIString>
                    <shape xsi:type="rectangle"/>
                </linkStyle>
            </masterSlide>
        </pathway>
    </PathwayList>
    <date>2020-05-25</date>
    <modelVersion>6</modelVersion>
    <saveLocation>D:\ver\bex\doc</saveLocation>
    <saveFile>D:\ver\bex\doc\bdd-types.vue</saveFile>
</LW-MAP>
