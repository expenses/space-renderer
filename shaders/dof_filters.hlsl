/********************************************************************/
/********************************************************************/
/*         Generated Filter by CircularDofFilterGenerator tool      */
/*     Copyright (c)     Kleber A Garcia  (kecho_garcia@hotmail.com)*/
/*       https://github.com/kecho/CircularDofFilterGenerator        */
/********************************************************************/
/********************************************************************/
/**
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE. 
**/
static const uint KERNEL_RADIUS = 8;
static const uint KERNEL_COUNT = 17;
static const float4 Kernel0BracketsRealXY_ImZW = float4(-0.018239,0.633946,-0.009345,0.366969);
static const float2 Kernel0Weights_RealX_ImY = float2(1.621035,-2.105439);
static const float4 Kernel0_RealX_ImY_RealZ_ImW[] = {
	float4(/*XY: Non Bracketed*/0.004963,-0.000906,/*Bracketed WZ:*/0.036600,0.022997),
	float4(/*XY: Non Bracketed*/-0.000375,-0.009345,/*Bracketed WZ:*/0.028180,0.000000),
	float4(/*XY: Non Bracketed*/-0.015297,-0.004584,/*Bracketed WZ:*/0.004642,0.012976),
	float4(/*XY: Non Bracketed*/-0.018239,0.017258,/*Bracketed WZ:*/0.000000,0.072494),
	float4(/*XY: Non Bracketed*/0.001641,0.036328,/*Bracketed WZ:*/0.031360,0.124461),
	float4(/*XY: Non Bracketed*/0.031713,0.036701,/*Bracketed WZ:*/0.078795,0.125477),
	float4(/*XY: Non Bracketed*/0.055303,0.022179,/*Bracketed WZ:*/0.116007,0.085905),
	float4(/*XY: Non Bracketed*/0.067107,0.006418,/*Bracketed WZ:*/0.134627,0.042956),
	float4(/*XY: Non Bracketed*/0.070245,0.000000,/*Bracketed WZ:*/0.139577,0.025466),
	float4(/*XY: Non Bracketed*/0.067107,0.006418,/*Bracketed WZ:*/0.134627,0.042956),
	float4(/*XY: Non Bracketed*/0.055303,0.022179,/*Bracketed WZ:*/0.116007,0.085905),
	float4(/*XY: Non Bracketed*/0.031713,0.036701,/*Bracketed WZ:*/0.078795,0.125477),
	float4(/*XY: Non Bracketed*/0.001641,0.036328,/*Bracketed WZ:*/0.031360,0.124461),
	float4(/*XY: Non Bracketed*/-0.018239,0.017258,/*Bracketed WZ:*/0.000000,0.072494),
	float4(/*XY: Non Bracketed*/-0.015297,-0.004584,/*Bracketed WZ:*/0.004642,0.012976),
	float4(/*XY: Non Bracketed*/-0.000375,-0.009345,/*Bracketed WZ:*/0.028180,0.000000),
	float4(/*XY: Non Bracketed*/0.004963,-0.000906,/*Bracketed WZ:*/0.036600,0.022997)
};
static const float4 Kernel1BracketsRealXY_ImZW = float4(-0.047314,1.023326,-0.039155,0.849239);
static const float2 Kernel1Weights_RealX_ImY = float2(-0.280860,-0.162882);
static const float4 Kernel1_RealX_ImY_RealZ_ImW[] = {
	float4(/*XY: Non Bracketed*/-0.001475,-0.020410,/*Bracketed WZ:*/0.044794,0.022073),
	float4(/*XY: Non Bracketed*/-0.013320,0.023855,/*Bracketed WZ:*/0.033219,0.074197),
	float4(/*XY: Non Bracketed*/0.034763,-0.004855,/*Bracketed WZ:*/0.080207,0.040389),
	float4(/*XY: Non Bracketed*/-0.018694,-0.039155,/*Bracketed WZ:*/0.027968,0.000000),
	float4(/*XY: Non Bracketed*/-0.047314,0.020606,/*Bracketed WZ:*/0.000000,0.070371),
	float4(/*XY: Non Bracketed*/0.002048,0.059024,/*Bracketed WZ:*/0.048238,0.115609),
	float4(/*XY: Non Bracketed*/0.050457,0.041030,/*Bracketed WZ:*/0.095543,0.094420),
	float4(/*XY: Non Bracketed*/0.067903,0.011703,/*Bracketed WZ:*/0.112591,0.059888),
	float4(/*XY: Non Bracketed*/0.070245,0.000000,/*Bracketed WZ:*/0.114880,0.046107),
	float4(/*XY: Non Bracketed*/0.067903,0.011703,/*Bracketed WZ:*/0.112591,0.059888),
	float4(/*XY: Non Bracketed*/0.050457,0.041030,/*Bracketed WZ:*/0.095543,0.094420),
	float4(/*XY: Non Bracketed*/0.002048,0.059024,/*Bracketed WZ:*/0.048238,0.115609),
	float4(/*XY: Non Bracketed*/-0.047314,0.020606,/*Bracketed WZ:*/0.000000,0.070371),
	float4(/*XY: Non Bracketed*/-0.018694,-0.039155,/*Bracketed WZ:*/0.027968,0.000000),
	float4(/*XY: Non Bracketed*/0.034763,-0.004855,/*Bracketed WZ:*/0.080207,0.040389),
	float4(/*XY: Non Bracketed*/-0.013320,0.023855,/*Bracketed WZ:*/0.033219,0.074197),
	float4(/*XY: Non Bracketed*/-0.001475,-0.020410,/*Bracketed WZ:*/0.044794,0.022073)
};
static const float4 Kernel2BracketsRealXY_ImZW = float4(-0.000825,0.503006,0.000000,0.127340);
static const float2 Kernel2Weights_RealX_ImY = float2(-0.366471,10.300301);
static const float4 Kernel2_RealX_ImY_RealZ_ImW[] = {
	float4(/*XY: Non Bracketed*/-0.000825,0.002179,/*Bracketed WZ:*/0.000000,0.017109),
	float4(/*XY: Non Bracketed*/0.000471,0.005155,/*Bracketed WZ:*/0.002576,0.040480),
	float4(/*XY: Non Bracketed*/0.004808,0.009153,/*Bracketed WZ:*/0.011198,0.071882),
	float4(/*XY: Non Bracketed*/0.013523,0.012724,/*Bracketed WZ:*/0.028523,0.099918),
	float4(/*XY: Non Bracketed*/0.026545,0.013927,/*Bracketed WZ:*/0.054413,0.109368),
	float4(/*XY: Non Bracketed*/0.041913,0.011680,/*Bracketed WZ:*/0.084964,0.091725),
	float4(/*XY: Non Bracketed*/0.056361,0.006841,/*Bracketed WZ:*/0.113689,0.053726),
	float4(/*XY: Non Bracketed*/0.066574,0.002011,/*Bracketed WZ:*/0.133992,0.015793),
	float4(/*XY: Non Bracketed*/0.070245,0.000000,/*Bracketed WZ:*/0.141290,0.000000),
	float4(/*XY: Non Bracketed*/0.066574,0.002011,/*Bracketed WZ:*/0.133992,0.015793),
	float4(/*XY: Non Bracketed*/0.056361,0.006841,/*Bracketed WZ:*/0.113689,0.053726),
	float4(/*XY: Non Bracketed*/0.041913,0.011680,/*Bracketed WZ:*/0.084964,0.091725),
	float4(/*XY: Non Bracketed*/0.026545,0.013927,/*Bracketed WZ:*/0.054413,0.109368),
	float4(/*XY: Non Bracketed*/0.013523,0.012724,/*Bracketed WZ:*/0.028523,0.099918),
	float4(/*XY: Non Bracketed*/0.004808,0.009153,/*Bracketed WZ:*/0.011198,0.071882),
	float4(/*XY: Non Bracketed*/0.000471,0.005155,/*Bracketed WZ:*/0.002576,0.040480),
	float4(/*XY: Non Bracketed*/-0.000825,0.002179,/*Bracketed WZ:*/0.000000,0.017109)
};
