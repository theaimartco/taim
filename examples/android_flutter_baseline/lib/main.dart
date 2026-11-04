import 'package:flutter/material.dart';

// Functional counterpart of android_counter. Uses platform/system fallback fonts,
// native Android IME, a normal/password field, multiline notes and retained state.
void main() => runApp(const BaselineApp());
class BaselineApp extends StatelessWidget {
  const BaselineApp({super.key});
  @override Widget build(BuildContext context) => MaterialApp(
    restorationScopeId: 'baseline',
    title: 'Flutter Counter',
    theme: ThemeData(colorSchemeSeed: const Color(0xff2563eb)),
    darkTheme: ThemeData(brightness: Brightness.dark, colorSchemeSeed: const Color(0xff2563eb)),
    home: const CounterPage(),
  );
}
class CounterPage extends StatefulWidget {
  const CounterPage({super.key});
  @override State<CounterPage> createState() => _CounterPageState();
}
class _CounterPageState extends State<CounterPage> with RestorationMixin {
  final count = RestorableInt(0);
  final message = RestorableTextEditingController();
  final password = TextEditingController(); // Deliberately not restored.
  final notes = TextEditingController(text: 'Multiline notes\nदूसरी पंक्ति');
  @override void initState() { super.initState(); WidgetsBinding.instance.addPostFrameCallback((_) => debugPrint('TAIM_FLUTTER_BASELINE_READY')); }
  @override String get restorationId => 'counter';
  @override void restoreState(RestorationBucket? oldBucket, bool initialRestore) {
    registerForRestoration(count, 'count');
    registerForRestoration(message, 'message');
  }
  @override void dispose() { count.dispose(); message.dispose(); password.dispose(); notes.dispose(); super.dispose(); }
  @override Widget build(BuildContext context) => Scaffold(body: SafeArea(child: SingleChildScrollView(
    padding: const EdgeInsets.all(32),
    child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const Text('Flutter on Android', style: TextStyle(fontSize: 28)),
      const SizedBox(height:16),
      Text('Count: ${count.value}', style: const TextStyle(fontSize:24)),
      const SizedBox(height:16),
      Align(alignment:Alignment.centerLeft,child:FilledButton(onPressed:()=>setState(()=>count.value++),child:const Text('Increment'))),
      const SizedBox(height:16),
      const Text('System fonts: नमस्ते · مرحبا · emoji 🌍'),
      const SizedBox(height:16),
      TextField(controller:message.value,decoration:const InputDecoration(border:OutlineInputBorder(),hintText:'Type here / यहाँ लिखें'),onChanged:(_)=>setState((){})),
      const SizedBox(height:16),
      Text('Message: ${message.value.text}'),
      const SizedBox(height:16),
      TextField(controller:password,obscureText:true,enableSuggestions:false,autocorrect:false,enableIMEPersonalizedLearning:false,autofillHints:const[AutofillHints.password],decoration:const InputDecoration(border:OutlineInputBorder(),hintText:'Password (private input)')),
      const SizedBox(height:16),
      TextField(controller:notes,minLines:3,maxLines:3,decoration:const InputDecoration(border:OutlineInputBorder())),
      const SizedBox(height:16),
      const Text('Background and reopen: the count should survive surface recreation.'),
    ]),
  )));
}
