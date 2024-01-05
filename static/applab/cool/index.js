////////////////////// STUFF TO READ //////////////////////////
// feel free to use this code or the app whereever you want (thats kinda the spirit of the remix button, isn't it?)
// if you want to suggest a new sprite, let me know at school
// you can find this game at (ethan.ws/cgtp) and (bit.do/cgtp)
// this game works on mobile! that's pretty cool right?

//////////////////////// STUFF THAT WILL GET FIXED LATER ///////////////
// add better design and a night theme 

/////////////////////////// CONSTANTS ////////////////////////////////
var HALF_TAU = Math.PI;
var TAU = 2*HALF_TAU;
var CLEAR = rgb(0, 0, 0, 0);
//var VIRTUAL_WIDTH = 100;
//var VIRTUAL_HEIGHT = 100;
var FOOD_MASS = 5;
var FOOD_RADIUS = radius(FOOD_MASS);
var MAX_FOOD_PER_PLAYER = 5;
var TRANSITION_DURATION = 750;
var PLAYER_SPEED = 20/1000;

// set up the colro sliders
var imageSelectElements = [
  "spriteDropdown",
];

var colorSelectElements = [
  "redSlider",
  "greenSlider",
  "blueSlider",
  "redInput",
  "greenInput",
  "blueInput",
];



////////////////////////////// SETUP //////////////////////////////////
setStyle("currentPlayers", "z-index: 999;");
setStyle("pingLabel", "z-index: 999;");
setStyle("framesLabel", "z-index: 999");


cleanupPlayersTable();



var schedule = new WriteSchedule();

var sampleColor;

var eatenPlayers = {};
var players = {};
var lastPlayers = {}; // the last value of the players variable
var food = {};


var myRecord; // used for display, and collision checking
// our copy of myRecord should be the same as our opponetns players[<our username>]

var selectedSpriteIndex = 0;

var pingMean = new LastNMean(3, 300);
var framesMean = new LastNMean(50, 10);
var lastFrame = getTime();

onEvent("main", "keydown", function(event) {
  if (event.key == "Right" || event.key == "d" || event.key == "e") {
    setPlayerAngle(0);
  } else if (event.key == "Up" || event.key == "w" || event.key == ",") {
    setPlayerAngle(TAU/4);
  } else if (event.key == "Left" || event.key == "a") {
    setPlayerAngle(TAU/2);
  } else if (event.key == "Down" || event.key == "s" || event.key == "o") {
    setPlayerAngle(3*TAU/4);
  }
});


onEvent("main", "mousemove", function(event) {
  // figure out the new angle we have to travel
  var dx = event.x - 160;
  var dy = 225 - event.y;
  var angle = Math.atan2(dy, dx);

  setPlayerAngle(angle);
});



function setPlayerAngle(angle) {
  var currentTime = getTime();
  
  var pos = positionAtTime(myRecord, currentTime);

  var hyp = getSpeed(myRecord)*currentTime;
  
  var dx = hyp*Math.cos(angle + TAU/2);
  var dy = hyp*Math.sin(angle + TAU/2);
  
  myRecord.x = pos[0] + dx;
  myRecord.y = pos[1] - dy;
  myRecord.angle = angle;

  updateMyRecord();
}



onEvent("playGameButton", "click", function() {
  var username = getText("usernameInput");
  if (!isUsernameValid(username)) {
    showElement("playGameErrorLabel");
    return;
  }
  
  setPreferences();

  readRecords("players", {}, function(otherPlayers) {
    // check that nobody has our username
    for (var i=0; i<otherPlayers.length; i++) {
      if (otherPlayers[i].username == username) {
        showElement("usernameInUseError");
        return;
      }
    }
    
    var recordToCreate = {
      username: username,
      x: randomNumber(0, 99), 
      y: randomNumber(0, 99),
      angle: TAU*Math.random(), // gives me a random angle
	    isColor: getChecked("colorButton"),
	    displayInfo: getChecked("colorButton")? rgb(sampleColor.r, sampleColor.g, sampleColor.b) : dropdownIndexToUrl(selectedSpriteIndex),
	    mass: 20,
	    sentTime: getTime(),
	    radiusMult: getChecked("colorButton")? 3/2 : 1,
    };

    
    
    schedule.add(createRecord, "players", recordToCreate, function (record) {
      myRecord = record;
  
      setScreen("main");
      createCanvas("foodCanvas", 500, 500);
      setStyle("foodCanvas", "pointer-events: none;");
      updateFoodCanvas();
      
//      createCanvas("pingStick", 320, 450);
      

      readRecords("players", {}, function (records) {
        for (var i=0; i<records.length; i++) {
          playerRecordEventHandler(records[i], "create");
        }
        
        onRecordEvent("players", playerRecordEventHandler);
  
        onRecordEvent("food", function(foodRecord, type) {
          if (type == "create") {
            food[foodRecord.id] = foodRecord;
            //createFood(foodRecord);
            updateFoodCanvas();
          } else if (type == "delete") {
            // delete the food from our collision stuff
            delete food[foodRecord.id];
            //deleteElement("#"+foodRecord.id);
            updateFoodCanvas();
          }
        }, true);
        
        timedLoop(6000, spawnFood);
        timedLoop(35, updateScreen);
//        timedLoop(5000, updateMyRecord); // for sentTime stuff
        timedLoop(1000, updateMyRecord); // for sentTime stuff
      });
    });
  });
});


function playerRecordEventHandler(playerRecord, type) {
  var id = playerRecord.id;
  if (type == "create") {
    // we will need to create a new sprite
    createPlayer(playerRecord);
    players[id] = playerRecord;
    lastPlayers[id] = playerRecord;
    
  } else if (type == "update") {
    if (playerRecord.id == myRecord.id) {
      pingMean.add(getTime()-myRecord.sentTime);
      setText("pingLabel", Math.round(pingMean.read()) + " ping");
    }
    lastPlayers[id] = players[id];
    players[id] = playerRecord;

  } else if (type == "delete") {
    // we will need to delete the current sprite we have for this player
    deleteElement("@"+id);
    delete players[id];
  }
  
  updateCurrentPlayers();
}


function updateScreen() {
  var currentTime = getTime();
  
  framesMean.add(currentTime-lastFrame);
  lastFrame = currentTime;
  setText("framesLabel", Math.round(1000/framesMean.read()) + " frames");
  
  var myPos = wrap(positionAtTime(myRecord, currentTime));
  var myXBeforeMap = myPos[0];
  var myYBeforeMap = myPos[1];
  var myRadius = radius(myRecord.mass);

  var scale = 5;


  var myApparentPos = playerPosition(myRecord.id, currentTime);
  var myApparentX = myApparentPos[0];
  var myApparentY = myApparentPos[1];
  var apparentRadius = radius(players[myRecord.id].mass);

//  setActiveCanvas("pingStick");
//  clearCanvas();
//  setStrokeColor("blue");
//  setStrokeWidth(4);
//  line(160, 225, scale*(myApparentX - myXBeforeMap) + 160, scale*(myApparentY - myYBeforeMap) + 225);

  // the onscreen size needs to change
  
  setPosition("background", 
    // the background stays at (0,0) so plug those coords into the mapping function
    -scale*myXBeforeMap + 160,
    -scale*myYBeforeMap + 225,
    scale*100, 
    scale*100
  );
  
  setPosition("foodCanvas", 
    // the background stays at (0,0) so plug those coords into the mapping function
    -scale*myXBeforeMap + 160,
    -scale*myYBeforeMap + 225,
    scale*100, 
    scale*100
  );

  // move all of the players
  for (var playerId in players) {
    var 
      player = players[playerId],
      pos = playerPosition(playerId, currentTime),
      //pos = wrap(positionAtTime(players[playerId], currentTime)),
      beforeMapX = pos[0],
      beforeMapY = pos[1],
    
      afterMapX = scale*(beforeMapX - myXBeforeMap) + 160,
      afterMapY = scale*(beforeMapY - myYBeforeMap) + 225,
      otherRadius = radius(player.mass),
      r = scale*otherRadius*player.radiusMult; // just for display

    // TODO: add name labels
    setPosition("@"+playerId, afterMapX-r, afterMapY-r, 2*r, 2*r);

    if (playerId !== myRecord.id && distance(myApparentX, myApparentY, beforeMapX, beforeMapY) < apparentRadius + otherRadius) {
      if (myRadius < otherRadius) {
        deleteRecord("players", myRecord, doNothing); // no schedule because we want to finish before throw
        setText("deathMessage", "You were eaten by "+player.username+". Refresh the page to respawn.");
        setScreen("lose");
        throw "TODO: add a respawn button";
      } else if (!eatenPlayers[playerId]) {
        
        updateMass(myRecord.mass+player.mass);

        eatenPlayers[playerId] = true;
      }
    }
  }
  
  // move all of the food
  for (var foodId in food) {
    var item = food[foodId];

    // lets try to eat the piece of food
    if (!item.hasEaten && distance(myApparentX, myApparentY, item.x, item.y) < apparentRadius + FOOD_RADIUS) {

      updateMass(myRecord.mass+FOOD_MASS);
      
      schedule.add(deleteRecord, "food", {id:foodId}, () => {});
      item.hasEaten = true;
    }
  }
}

function updateMass(newMass) {
  // we need to update the posiiton because speed depends on mass
  var currentTime = getTime();
  var pos = positionAtTime(myRecord, currentTime);
  
  myRecord.mass = newMass;
  var newSpeed = getSpeed(myRecord);
  
  myRecord.x = pos[0] - Math.cos(myRecord.angle)*newSpeed*currentTime;
  myRecord.y = pos[1] + Math.sin(myRecord.angle)*newSpeed*currentTime;
  
  updateMyRecord();
}

function updateMyRecord() {
  myRecord.sentTime = getTime();
  schedule.updatePlayerRecord(myRecord);
}

function updateFoodCanvas() {
  setActiveCanvas("foodCanvas");
  clearCanvas();
  setStrokeColor(CLEAR);
  
  for (var id in food) {
    var item = food[id];
    setFillColor(item.color);
    circle(5*item.x, 5*item.y, 5*FOOD_RADIUS);
  }
}


function createPlayer(record) {
  // TODO: add name labels
  var id = "@"+record.id; // so we don't get id conflicts with our own elements
  button(id, record.username);
  setSize(id, 0, 0);
  setProperty(id, "background-color", CLEAR);
  setProperty(id, "text-color", "black");

  if (record.isColor) {
//    setProperty(id, "image", "icon://fa-circle");
//    setProperty(id, "icon-color", record.displayInfo);
    const element = document.getElementById(id)
    element.style.clipPath = 'circle(50%)';
    element.style.backgroundColor = record.displayInfo;
  } else {
    setProperty(id, "image", record.displayInfo);
  }
  
  setStyle(id, "pointer-events: none;");
}

var uid;
try {
  uid = getUserId();
} catch (e) {
  uid = "none";
}

readRecords("preferences", {uid:uid}, function(records) {
  if (records.length === 0) {
    // there are no prefererences set up
    randomiseColor();
  } else {

    // set it up based on the preferences
    var record = records[0];
    setText("usernameInput", record.username);
    
    if (record.doesPerferColor) {
      sampleColor = JSON.parse(record.displayInfo);
      setColorSample();
    } else {
      randomiseColor();
      
      selectedSpriteIndex = record.displayInfo;
      setImageURL("imageSample", dropdownIndexToUrl(selectedSpriteIndex));
      setProperty("spriteDropdown", "index", record.displayInfo);

      setChecked("imageButton", true);
      onImageButtonClick();
    }
  }
});

function setPreferences() {
  var displayInfo = getChecked("colorButton")? JSON.stringify(sampleColor) : selectedSpriteIndex;
  var newRecord = {
    uid: uid,
    doesPerferColor: getChecked("colorButton"),
    displayInfo: displayInfo,
    username: getText("usernameInput"),
  };
  
  readRecords("preferences", {uid: uid}, function(records) {
    if (records.length === 0) { // we are creating it for the firs ttime
      schedule.add(createRecord, "preferences", newRecord, () => {});
    } else {
      newRecord.id = records[0].id;
      schedule.add(updateRecord, "preferences", newRecord, () => {});
    }
  });
}




function randomiseColor() {
  sampleColor = {
    r: randomNumber(0, 255),
    g: randomNumber(0, 255),
    b: randomNumber(0, 255),
  };
  setColorSample();
}


function onColorButtonClick() {
  var i;
  
  setColorSample();
  
  for (i=0; i<imageSelectElements.length; i++) {
    hideElement(imageSelectElements[i]);
  }
  for (i=0; i<colorSelectElements.length; i++) {
    showElement(colorSelectElements[i]);
  }
}

function onImageButtonClick() {
  var i;
  
  setImageURL("imageSample", dropdownIndexToUrl(orZero(selectedSpriteIndex)));
  
  for (i=0; i<colorSelectElements.length; i++) {
    hideElement(colorSelectElements[i]);
  }
  for (i=0; i<imageSelectElements.length; i++) {
    showElement(imageSelectElements[i]);
  }
}

// hide all of the image settings and show the color settings
onEvent("colorButton", "click", onColorButtonClick);

// hide all of the color settings and show the image ones
// show all of the image settings
onEvent("imageButton", "click", onImageButtonClick);

// i know i can create these on-events in a loop, but
// being explicit is more clear
onEvent("redInput", "input", function() {
  sampleColor.r = orZero(getText("redInput"));
  setColorSample();
});
onEvent("greenInput", "input", function() {
  sampleColor.g = orZero(getText("greenInput"));
  setColorSample();
});
onEvent("blueInput", "input", function() {
  sampleColor.b = orZero(getNumber("blueInput"));
  setColorSample();
});

onEvent("redSlider", "input", function() {
  sampleColor.r = getNumber("redSlider");
  setColorSample();
});
onEvent("greenSlider", "input", function() {
  sampleColor.g = getNumber("greenSlider");
  setColorSample();
});
onEvent("blueSlider", "input", function() {
  sampleColor.b = getNumber("blueSlider");
  setColorSample();
});


onEvent("spriteDropdown", "change", function() {
  var newIndex = getProperty("spriteDropdown", "index");
  selectedSpriteIndex = newIndex;
  setImageURL("imageSample", dropdownIndexToUrl(selectedSpriteIndex));
});


function dropdownIndexToUrl(index) {
  var skinNames = ["cell", "diamondSword", "duckDuckGo", "elsa", "polandBall", "scp", "tidePod"];
  
  var skinName = skinNames[index];
  if (!skinName) throw "invalid skin index";
  
  return skinName+".png";
}

function setColorSample() {
  // sets the color sample based on sampleColor
  setText("redInput", orBlank(sampleColor.r));
  setText("greenInput", orBlank(sampleColor.g));
  setText("blueInput", orBlank(sampleColor.b));

  setNumber("redSlider", sampleColor.r);
  setNumber("greenSlider", sampleColor.g);
  setNumber("blueSlider", sampleColor.b);

  showElement("imageSample");
//  setImageURL("imageSample", "icon://fa-circle");
//  setProperty("imageSample", "icon-color", rgb(sampleColor.r, sampleColor.g, sampleColor.b));
  const element = document.getElementById('imageSample')
  element.style.clipPath = 'circle(50%)';
  element.style.backgroundColor = rgb(sampleColor.r, sampleColor.g, sampleColor.b);
}

function orBlank(x) {
  return (x === 0)? "" : x;
}

function orZero(x) {
  return parseInt(x)? x : 0;
}

function isUsernameValid(username) {
  // we want at least one printable character. that regex doesn't have to match the whole string
  return username.match(/[a-zA-Z0-9]/);
}

function updateCurrentPlayers() {
  var playersArray = Object.keys(players);
  
  playersArray.sort(function(a, b) {
    if (players[a].mass < players[b].mass) {
      return 1;
    } else {
      return -1;
    }
  });
  
  var string = "top players:";
  
  for (var i=0; i<playersArray.length; i++) {
    var p = players[playersArray[i]];
    string += "\n"+(i+1)+": "+p.username+" - "+p.mass;
  }

  setText("currentPlayers", string);
}


function LastNMean(n, init) {
  this.last = [];
  this.mean;
  
  this.read = function() {
    return this.mean;
  };
  
  this.add = function(value) {
    this.last.push(value);
    if (this.last.length > n) {
      this.last.shift();
    }
    
    // calculate the mean here so this.read can be cheap
    var sum = 0;
    for (var i=0; i<this.last.length; i++) sum += this.last[i];
    this.mean = sum / this.last.length;  
  };
  
  this.add(init);
}

function WriteSchedule() {
  // this is to prevent this app from dying from code.org's data rate limit
  // only allows the client to write to a table every 200 millisceonds
  this.buf = [];
  this.lastUpdated = 0;
  this.playerUpdateRecord = null;
  
  this.updatePlayerRecord = function (newRecord) {
    // yes, overwriting the old value of playerUpdateRecord is totally ok
    this.playerUpdateRecord = JSON.parse(JSON.stringify(newRecord));
    this.update();
  };
  
  this.add = function(f, table, record, callback) {
    record = JSON.parse(JSON.stringify(record)); // we want to do a deep copy
    
    this.buf.unshift(function() { f(table, record, callback) });
    this.update();
  };
  
  this.update = function() {
    // tries to send the update 
    if (this.lastUpdated+200 > getTime()) return;
    
    // priority is updates from the normal buf (removing food, spawning food)
    
    if (this.buf.length > 0) {
      this.buf.pop()();
      this.lastUpdated = getTime();
      
    } else if (this.playerUpdateRecord) {
      updateRecord("players", this.playerUpdateRecord, () => {});
      
      this.lastUpdated = getTime();
      this.playerUpdateRecord = null;
    }
  };
  
  var that = this;
  timedLoop(100, function() {
    that.update();
  });
}

function radius(mass) {
  return Math.sqrt(mass/HALF_TAU);
}

function randomColor() {
  return rgb(
    randomNumber(0, 255),
    randomNumber(0, 255),
    randomNumber(0, 255)
  );
}

function doNothing() {}


function wrap(pos) {
  pos[0] %= 100;
  if (pos[0] < 0) pos[0] += 100;
  pos[1] %= 100;
  if (pos[1] < 0) pos[1] += 100;
  
  // wrap (make sure all coordinates are within [0, VIRTUAL_WIDTH))
  return pos;
}

function playerPosition(id, currentTime) {
  var cur = players[id];
  var curPos = positionAtTime(cur, currentTime);

  if (cur.sentTime + TRANSITION_DURATION < currentTime) return wrap(curPos);
  
  var old = lastPlayers[id];
  var oldPos = positionAtTime(old, currentTime);

  var multFactor = (currentTime-old.sentTime-TRANSITION_DURATION)/(cur.sentTime-old.sentTime);

  var dx = curPos[0] - oldPos[0];
  var dy = curPos[1] - oldPos[1];

  return wrap([
    oldPos[0] + dx*multFactor,
    oldPos[1] + dy*multFactor
  ]);
}



function positionAtTime(record, time) {
  //var speed = PLAYER_SPEED;
  
  // TODO: make up a better equation
  //var speed = makeNonNegative(0.0156208 - 0.0000310417 * record.mass);
  //var speed = makeNonNegative(0.0111 - 0.000055*record.mass);
  var speed = getSpeed(record);
  
  var x = record.x + time*speed*Math.cos(record.angle);
  var y = record.y - time*speed*Math.sin(record.angle);
  
  return [x, y];
}

function makeNonNegative(x) {
  return (x<0)? 0 : x;
}

function getSpeed(record) {
  // TODO: improve
  return makeNonNegative(0.0156208 - 0.0000310417 * record.mass);
}

function distance(x0, y0, x1, y1) {
  var dx = x1 - x0;
  var dy = y1 - y0;
  
  return Math.sqrt(dx*dx + dy*dy);
}


function spawnFood() {
  // figure how much food is on the map already
  // there can be up to 5 food items per player
  var playersPresent = Object.keys(players).length;
  
  var maxFood = playersPresent * MAX_FOOD_PER_PLAYER;
  var foodPresent = Object.keys(food).length;
  
  if (foodPresent < maxFood) {
    var record = {
      x: randomNumber(FOOD_RADIUS, 100-FOOD_RADIUS),
      y: randomNumber(FOOD_RADIUS, 100-FOOD_RADIUS),
      color: randomColor(),
    };
    
    schedule.add(createRecord, "food", record, () => {});
  }
}


function cleanupPlayersTable() {
  // we cannot use the players object because this is run before it is created
  readRecords("players", {}, function (records) {
    for (var i=0; i<records.length; i++) {
//      if (getTime() - records[i].sentTime > 10*1000) {
      if (getTime() - records[i].sentTime > 1000) {
        console.log("deleted from being inactive" +(getTime() - records[i].sentTime) );
        schedule.add(deleteRecord, "players", records[i], () => {});
      }
    }
  });
}

