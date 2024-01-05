var TEXT = ["when", "in", "the", "course", "of", "human", "events", "it", "becomes", "necessary", "for", "one", "people", "to", "dissolve", "the", "political", "bands", "which", "have", "connected", "them", "with", "another", "and", "to", "assume", "among", "the", "powers", "of", "the", "earth", "the", "separate", "and", "equal", "station", "to", "which", "the", "laws", "of", "nature", "and", "of", "natures", "god", "entitle", "them", "a", "decent", "respect", "to", "the", "opinions", "of", "mankind", "requires", "that", "they", "should", "declare", "the", "causes", "which", "impel", "them", "to", "the", "separation", "we", "hold", "these", "truths", "to", "be", "selfevident", "that", "all", "men", "are", "created", "equal", "that", "they", "are", "endowed", "by", "their", "creator", "with", "certain", "unalienable", "rights", "that", "among", "these", "are", "life", "liberty", "and", "the", "pursuit", "of", "happiness", "that", "to", "secure", "these", "rights", "governments", "are", "instituted", "among", "men", "deriving", "their", "just", "powers", "from", "the", "consent", "of", "the", "governed", "that", "whenever", "any", "form", "of", "government", "becomes", "destructive", "of", "these", "ends", "it", "is", "the", "right", "of", "the", "people", "to", "alter", "or", "to", "abolish", "it", "and", "to", "institute", "new", "government", "laying", "its", "foundation", "on", "such", "principles", "and", "organizing", "its", "powers", "in", "such", "form", "as", "to", "them", "shall", "seem", "most", "likely", "to", "effect", "their", "safety", "and", "happiness", "prudence", "indeed", "will", "dictate", "that", "governments", "long", "established", "should", "not", "be", "changed", "for", "light", "and", "transient", "causes", "and", "accordingly", "all", "experience", "hath", "shown", "that", "mankind", "are", "more", "disposed", "to", "suffer", "while", "evils", "are", "sufferable", "than", "to", "right", "themselves", "by", "abolishing", "the", "forms", "to", "which", "they", "are", "accustomed", "but", "when", "a", "long", "train", "of", "abuses", "and", "usurpations", "pursuing", "invariably", "the", "same", "object", "evinces", "a", "design", "to", "reduce", "them", "under", "absolute"];
var LINEWIDTH = 4;
var spelled = [];
var startTime;
var timerUpdateLoop;
var wordsCorrect = 0;
init();

onEvent("main", "keydown", function(event) {
  var word; // to declare outside of the if's
  
  if (event.keyCode == 32) {
    // the space was hit move on to the next word
    word = stripSpaces(getText("input"));
    
    if (word == TEXT[spelled.length]) {
      wordsCorrect++;
    }
    
    spelled.push(word);
    
    //wait(100);
    setText("input", "");
    putWords();
  } else if (event.keyCode == 8 && stripSpaces(getText("input")) === "" && spelled.length > 0) {
    // the user hit a backspace to move on to the previous word
    // this part is when the user wants to edit a previously spelled word
    // the word they want to edit
    word = spelled.pop(); 
    if (word == TEXT[spelled.length]) {
      wordsCorrect--;
    }
    
    wait(100);
    setText("input", word);
    putWords();
  }
});

function stripSpaces(oldString) {
  var newString = "";
  
  for (var i = 0; i < oldString.length; i++) {
    if (oldString[i] != " ") {
      newString += oldString[i];
    } 
  }
  return newString;
}

function putWords() {
  // puts the words in `display` centered in the middle 
  // the display should have places for three lines 
  // get the actual string
  var cursorIndex = spelled.length;
  var startPlace = cursorIndex - cursorIndex%LINEWIDTH;
  
  var previousLine;
  if (startPlace>LINEWIDTH-1) {
    previousLine = lineString(startPlace-LINEWIDTH)+"\n";
  } else {
    previousLine = "\n";
  }
  
  var currentLine = lineString(startPlace)+"\n";
  var nextLine = lineString(startPlace+LINEWIDTH);

  setText("display", previousLine + currentLine + nextLine);
}

function lineString(start) {
  // inserts all of the brackets and stuff and makes the string
  var currentIndex = spelled.length;
  
  var string = "";
  
  for (var i = start; i< start+LINEWIDTH; i++) {
    if (i == currentIndex) {
      // we are on the word that is being typed
      string += "❯";
 
    } else if (TEXT[i] != spelled[i] && spelled[i] !== undefined) {
      // they mispelled the word, put an x
      string += "᙮";
    } else {
      string += " ";
      // just a regular space that doesn't need replacing
    }
    
    string += TEXT[i];
  }
  return string;
}

function init() {
  // set up the timer and stuff
  startTime = getTime();
  timerUpdateLoop = timedLoop(500, function() {
    // we don't have to update it that often
    var secondsPassed = Math.round((getTime()-startTime)/1000);
    
    setText("timer", 60-secondsPassed);
    setText("wpm", Math.round(wordsCorrect*60/secondsPassed));
  });
  
  setTimeout(function() {
    // TODO: make this go to the display score screen
    stopTimedLoop(timerUpdateLoop); // i like to be hygenic
  }, 60000);
  
  putWords();
}

function drawHighScores() {
  readRecords("Scores", {}, function(scores) {
    var scoreList = scores;
    scoreList.sort(function(a, b) {
      // sort by key
      if (a.score < b.score) return -1;
      if (a.score > b.score) return 1;
      return 0;
    });
    
    var string = "";
    for (var i = 0; i < scoreList.length; i++) {
      string += i + ") " + scoreList[i].name + " " + scoreList[i].score + "\n";
    }
    
    setText("HighScoreLabel", string);
    
  });
  
}

function wait(duration) {
  var startTime = getTime();
  while (startTime + duration > getTime()) {}
}

